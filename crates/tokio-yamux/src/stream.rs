//! The substream, the main interface is AsyncRead/AsyncWrite

use bytes::BytesMut;
use futures::{
    Stream,
    channel::mpsc::{Receiver, UnboundedSender},
    stream::FusedStream,
    task::Waker,
};

use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use futures::future;
use log::{debug, trace};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::{
    StreamId,
    config::INITIAL_STREAM_WINDOW,
    error::Error,
    frame::{Flag, Flags, Frame, Type},
};

/// The substream
#[derive(Debug)]
pub struct StreamHandle {
    id: StreamId,
    state: StreamState,

    max_recv_window: u32,
    pub(crate) recv_window: u32,
    send_window: u32,
    read_buf: Vec<BytesMut>,

    // Send stream event to parent session
    unbound_event_sender: UnboundedSender<StreamEvent>,

    // Receive frame of current stream from parent session
    // (if the sender closed means session closed the stream should close too)
    frame_receiver: Receiver<Frame>,

    // when the cache is sent, a writable notification is issued
    writeable_wake: Option<Waker>,

    // when the cache is received by write, a readable notification is issued
    readable_wake: Option<Waker>,
}

impl StreamHandle {
    // Create a StreamHandle from session
    pub(crate) fn new(
        id: StreamId,
        unbound_event_sender: UnboundedSender<StreamEvent>,
        frame_receiver: Receiver<Frame>,
        state: StreamState,
        max_window_size: u32,
    ) -> StreamHandle {
        assert!(state == StreamState::Init || state == StreamState::SynReceived);
        StreamHandle {
            id,
            state,
            max_recv_window: max_window_size,
            recv_window: INITIAL_STREAM_WINDOW,
            send_window: INITIAL_STREAM_WINDOW,
            read_buf: Vec::new(),
            unbound_event_sender,
            frame_receiver,
            writeable_wake: None,
            readable_wake: None,
        }
    }

    /// Get the stream id
    pub fn id(&self) -> StreamId {
        self.id
    }
    /// Get the stream state
    pub fn state(&self) -> StreamState {
        self.state
    }
    /// Get the receive window size
    pub fn recv_window(&self) -> u32 {
        self.recv_window
    }
    /// Get the send window size
    pub fn send_window(&self) -> u32 {
        self.send_window
    }

    fn close(&mut self) -> Result<(), Error> {
        match self.state {
            StreamState::SynSent
            | StreamState::SynReceived
            | StreamState::Established
            | StreamState::Init => {
                self.state = StreamState::LocalClosing;
                self.send_close()?;
            }
            StreamState::RemoteClosing => {
                self.state = StreamState::Closed;
                self.send_close()?;
                let event = StreamEvent::Closed(self.id);
                self.unbound_send_event(event)?;
            }
            StreamState::Reset | StreamState::Closed => {
                self.state = StreamState::Closed;
                let event = StreamEvent::Closed(self.id);
                self.unbound_send_event(event)?;
            }
            StreamState::LocalClosing => {
                self.state = StreamState::Closed;
                let event = StreamEvent::Closed(self.id);
                self.unbound_send_event(event)?;
            }
        }
        Ok(())
    }

    fn send_go_away(&mut self) {
        self.state = StreamState::LocalClosing;
        let _ignore = self
            .unbound_event_sender
            .unbounded_send(StreamEvent::GoAway);
    }

    fn unbound_send_event(&mut self, event: StreamEvent) -> Result<(), Error> {
        self.unbound_event_sender
            .unbounded_send(event)
            .map_err(|_| Error::SessionShutdown)
    }

    #[inline]
    fn unbound_send_frame(&mut self, frame: Frame) -> Result<(), Error> {
        trace!(
            "stream-handle({}) send_frame ty={:?}, size={}",
            self.id,
            frame.ty(),
            frame.size()
        );
        let event = StreamEvent::Frame(frame);
        self.unbound_send_event(event)
    }

    // Send a window update
    pub(crate) fn send_window_update(&mut self) -> Result<(), Error> {
        let delta = receive_credit_delta(
            self.max_recv_window,
            self.recv_window,
            self.read_buf.iter().map(BytesMut::len),
        )?;

        // Check if we can omit the update
        let flags = self.get_flags();
        // Compare against half without rounding an odd window down. For an
        // odd ceiling, the first update is sent when credit reaches ceil(max / 2).
        if u64::from(delta) * 2 < u64::from(self.max_recv_window) && flags.value() == 0 {
            return Ok(());
        }
        let frame = Frame::new_window_update(flags, self.id, delta);
        self.unbound_event_sender
            .unbounded_send(StreamEvent::Frame(frame))
            .map_err(|_| Error::SessionShutdown)?;
        self.recv_window += delta;
        Ok(())
    }

    fn send_data(&mut self, data: &[u8]) -> Result<(), Error> {
        let flags = self.get_flags();
        let frame = Frame::new_data(flags, self.id, BytesMut::from(data));
        self.unbound_send_frame(frame)
    }

    fn send_close(&mut self) -> Result<(), Error> {
        let mut flags = self.get_flags();
        flags.add(Flag::Fin);
        let frame = Frame::new_window_update(flags, self.id, 0);
        self.unbound_send_frame(frame)
    }

    fn process_flags(&mut self, flags: Flags) -> Result<(), Error> {
        if flags.contains(Flag::Ack) && self.state == StreamState::SynSent {
            self.state = StreamState::Established;
        }
        if flags.contains(Flag::Fin) {
            match self.state {
                StreamState::Init
                | StreamState::SynSent
                | StreamState::SynReceived
                | StreamState::Established => {
                    self.state = StreamState::RemoteClosing;
                }
                StreamState::LocalClosing => {
                    self.state = StreamState::Closed;
                }
                _ => return Err(Error::UnexpectedFlag),
            }
        }
        if flags.contains(Flag::Rst) {
            self.state = StreamState::Reset;
            // Do NOT clear read_buf here: check_self_state delivers already-queued
            // bytes to the application first, then reports ConnectionReset once the
            // queue drains. Wiping the queue loses data legitimately received before
            // the reset (e.g. a peer that finishes its transfer and drops the
            // stream, which sends RST on Drop). The payload of a DATA+RST frame
            // itself is still discarded below.
        }
        Ok(())
    }

    fn get_flags(&mut self) -> Flags {
        match self.state {
            StreamState::Init => {
                self.state = StreamState::SynSent;
                Flags::from(Flag::Syn)
            }
            StreamState::SynReceived => {
                self.state = StreamState::Established;
                Flags::from(Flag::Ack)
            }
            _ => Flags::default(),
        }
    }

    fn handle_frame(&mut self, frame: Frame) -> Result<(), Error> {
        trace!(
            "stream-handle({}) handle_frame ty={:?}, size={}",
            self.id,
            frame.ty(),
            frame.size()
        );
        match frame.ty() {
            Type::WindowUpdate => {
                self.handle_window_update(&frame)?;
            }
            Type::Data => {
                self.handle_data(frame)?;
            }
            _ => {
                return Err(Error::InvalidMsgType);
            }
        }
        Ok(())
    }

    fn handle_window_update(&mut self, frame: &Frame) -> Result<(), Error> {
        self.process_flags(frame.flags())?;
        // Reject credit overflow before changing the send window.
        self.send_window = self
            .send_window
            .checked_add(frame.length())
            .ok_or(Error::InvalidMsgType)?;
        // wake writer continue
        if let Some(waker) = self.writeable_wake.take() {
            waker.wake()
        }
        Ok(())
    }

    fn handle_data(&mut self, frame: Frame) -> Result<(), Error> {
        self.process_flags(frame.flags())?;
        let length = frame.length();
        if length > self.recv_window {
            return Err(Error::RecvWindowExceeded);
        }

        let (_, body) = frame.into_parts();
        if let Some(data) = body {
            // yamux allows empty data frame
            // but here we just drop it
            // A DATA frame carrying RST is terminal too; process_flags has
            // already moved the stream to Reset, so discard its payload.
            if length > 0 && self.state != StreamState::Reset {
                self.read_buf.push(data);
            }
        }
        self.recv_window -= length;
        Ok(())
    }

    fn recv_frames(&mut self, cx: &mut Context) -> Result<bool, Error> {
        trace!("stream-handle({}) recv_frames", self.id);
        let mut has_new_frame = false;
        loop {
            match self.state {
                StreamState::RemoteClosing => {
                    return Err(Error::SubStreamRemoteClosing);
                }
                StreamState::Reset | StreamState::Closed => {
                    return Err(Error::SessionShutdown);
                }
                _ => {}
            }

            if self.frame_receiver.is_terminated() {
                self.state = StreamState::RemoteClosing;
                return Err(Error::SubStreamRemoteClosing);
            }

            match Pin::new(&mut self.frame_receiver).as_mut().poll_next(cx) {
                Poll::Ready(Some(frame)) => {
                    self.handle_frame(frame)?;
                    has_new_frame = true;
                }
                Poll::Ready(None) => {
                    self.state = StreamState::RemoteClosing;
                    return Err(Error::SubStreamRemoteClosing);
                }
                Poll::Pending => break,
            }
        }
        Ok(has_new_frame)
    }

    fn try_recv_frames(&mut self) -> Result<bool, Error> {
        let mut has_new_frame = false;
        loop {
            match self.state {
                StreamState::RemoteClosing => {
                    return Err(Error::SubStreamRemoteClosing);
                }
                StreamState::Reset | StreamState::Closed => {
                    return Err(Error::SessionShutdown);
                }
                _ => {}
            }

            if self.frame_receiver.is_terminated() {
                self.state = StreamState::RemoteClosing;
                return Err(Error::SubStreamRemoteClosing);
            }

            match self.frame_receiver.try_recv() {
                Ok(frame) => {
                    self.handle_frame(frame)?;
                    has_new_frame = true;
                }
                Err(futures::channel::mpsc::TryRecvError::Closed) => {
                    self.state = StreamState::RemoteClosing;
                    return Err(Error::SubStreamRemoteClosing);
                }
                Err(futures::channel::mpsc::TryRecvError::Empty) => break,
            }
        }
        Ok(has_new_frame)
    }

    fn recv_frames_wake(&mut self) -> Result<(), Error> {
        let buf_len = self.read_buf.len();
        let state = self.state;
        match self.try_recv_frames() {
            Ok(should_wake_read) => {
                // if state change to RemoteClosing, wake read
                // if read buf len change, wake read
                if (self.state == StreamState::RemoteClosing && state != StreamState::RemoteClosing)
                    || (should_wake_read && buf_len != self.read_buf.len())
                {
                    if let Some(waker) = self.readable_wake.take() {
                        waker.wake();
                    }
                }

                Ok(())
            }
            Err(e) => {
                // if state change to RemoteClosing, wake read
                if self.state == StreamState::RemoteClosing && state != StreamState::RemoteClosing {
                    if let Some(waker) = self.readable_wake.take() {
                        waker.wake();
                    }
                }

                Err(e)
            }
        }
    }

    /// Blocking variant of [`Self::recv_frames_wake`]: drains inbound frames and
    /// registers the current task's waker on `frame_receiver`, so that frames
    /// arriving later (e.g. window updates) wake this task.
    ///
    /// Use ONLY when no read task is currently parked; otherwise this would
    /// overwrite the reader's waker stored in `frame_receiver` and the read
    /// side could hang (see `test_write_side_does_not_overwrite_read_waker`).
    /// When no read task exists (e.g. one-way bulk send where the local app
    /// never reads), nobody else would process inbound frames, so the write
    /// path must register itself -- otherwise a window update arriving after
    /// `poll_write` parks on `send_window == 0` wakes nobody and the write
    /// side stalls forever.
    fn recv_frames_wake_blocking(&mut self, cx: &mut Context) -> Result<(), Error> {
        let buf_len = self.read_buf.len();
        let state = self.state;
        match self.recv_frames(cx) {
            Ok(should_wake_read) => {
                // if state change to RemoteClosing, wake read
                // if read buf len change, wake read
                if (self.state == StreamState::RemoteClosing && state != StreamState::RemoteClosing)
                    || (should_wake_read && buf_len != self.read_buf.len())
                {
                    if let Some(waker) = self.readable_wake.take() {
                        waker.wake();
                    }
                }

                Ok(())
            }
            Err(e) => {
                // if state change to RemoteClosing, wake read
                if self.state == StreamState::RemoteClosing && state != StreamState::RemoteClosing {
                    if let Some(waker) = self.readable_wake.take() {
                        waker.wake();
                    }
                }

                Err(e)
            }
        }
    }

    // Returns Ok(true) only if eof is reached.
    fn check_self_state(&mut self) -> io::Result<bool> {
        // if read buf is empty and state is close, return close error
        if self.read_buf.is_empty() {
            match self.state {
                StreamState::RemoteClosing | StreamState::Closed => {
                    debug!("closed(EOF)");
                    // an empty read indicates that EOF is reached.
                    Ok(true)
                }
                StreamState::Reset => {
                    debug!("connection reset");
                    Err(io::ErrorKind::ConnectionReset.into())
                }
                _ => Ok(false),
            }
        } else {
            Ok(false)
        }
    }

    /// Attempts to receive data on the socket, without removing that data from the queue,
    /// registering the current task for wakeup if data is not yet available.
    pub fn poll_peek(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<usize>> {
        if self.check_self_state()? {
            return Poll::Ready(Ok(0));
        }

        self.readable_wake = Some(cx.waker().clone());
        if let Err(Error::UnexpectedFlag | Error::RecvWindowExceeded | Error::InvalidMsgType) =
            self.recv_frames(cx)
        {
            // read flag error or read data error
            self.send_go_away();
            return Poll::Ready(Err(io::ErrorKind::InvalidData.into()));
        }

        if self.check_self_state()? {
            return Poll::Ready(Ok(0));
        }

        if self.read_buf.is_empty() {
            return Poll::Pending;
        }

        let mut total_read = 0;
        for read_buf in self.read_buf.iter() {
            let n = buf.remaining().min(read_buf.len());
            if n == 0 {
                break;
            }
            total_read += n;
            let b = &read_buf[..n];
            buf.put_slice(b);
        }

        trace!(
            "stream-handle({}) poll_peek self.read_buf.len={}, buf.len={}, n={}",
            self.id,
            self.read_buf.len(),
            buf.remaining(),
            total_read,
        );

        Poll::Ready(Ok(total_read))
    }

    /// Receives data on the socket from the remote address to which it is connected,
    /// without removing that data from the queue. On success, returns the number of bytes peeked.
    pub async fn peek(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut read_buf = ReadBuf::new(buf);
        future::poll_fn(|cx| self.poll_peek(cx, &mut read_buf)).await
    }
}

// Calculate the credit to advertise while keeping every conversion and
// subtraction checked. Keeping this separate makes impossible-to-allocate
// buffer sizes testable without weakening the stream's bounded buffering.
fn receive_credit_delta(
    max_recv_window: u32,
    recv_window: u32,
    mut buffered_lengths: impl Iterator<Item = usize>,
) -> Result<u32, Error> {
    let buffered = buffered_lengths
        .try_fold(0u32, |total, len| {
            total.checked_add(u32::try_from(len).ok()?)
        })
        .ok_or(Error::InvalidMsgType)?;
    max_recv_window
        .checked_sub(buffered)
        .and_then(|available| available.checked_sub(recv_window))
        .ok_or(Error::InvalidMsgType)
}

fn should_shrink_read_buffer(capacity: usize, len: usize) -> bool {
    capacity > 24 && capacity / (len + 1) > 4
}

impl AsyncRead for StreamHandle {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.check_self_state()? {
            return Poll::Ready(Ok(()));
        }

        self.readable_wake = Some(cx.waker().clone());
        if let Err(Error::UnexpectedFlag | Error::RecvWindowExceeded | Error::InvalidMsgType) =
            self.recv_frames(cx)
        {
            // read flag error or read data error
            self.send_go_away();
            return Poll::Ready(Err(io::ErrorKind::InvalidData.into()));
        }

        if self.check_self_state()? {
            return Poll::Ready(Ok(()));
        }

        if self.read_buf.is_empty() {
            return Poll::Pending;
        }

        let mut offset = None;
        let mut total_read = 0;
        for (index, read_buf) in self.read_buf.iter_mut().enumerate() {
            let n = buf.remaining().min(read_buf.len());
            if n == 0 {
                break;
            }
            buf.put_slice(&read_buf.split_to(n));
            if read_buf.is_empty() {
                offset = Some(index);
            }
            total_read += n;
        }
        if let Some(offset) = offset {
            self.read_buf.drain(..=offset);
            // Drop an unusually large stream queue after draining it; otherwise a burst
            // would keep its high-water allocation for the lifetime of this stream.
            if should_shrink_read_buffer(self.read_buf.capacity(), self.read_buf.len()) {
                self.read_buf.shrink_to_fit();
            }
        }

        trace!(
            "stream-handle({}) poll_read self.read_buf.len={}, buf.len={}, n={}",
            self.id,
            self.read_buf.len(),
            buf.remaining(),
            total_read,
        );

        match self.state {
            StreamState::RemoteClosing | StreamState::Closed | StreamState::Reset => {
                debug!("this branch should be unreachable")
            }
            _ => {
                if self.send_window_update().is_err() {
                    return Poll::Ready(Err(io::ErrorKind::BrokenPipe.into()));
                }
            }
        }

        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for StreamHandle {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        // https://github.com/driftluo/tentacle/issues/33
        // read frame from session is necessary.
        // The window update message of Yamux must be updated normally.
        // If the user only writes but does not read, the entire stream will be stuck.
        // To avoid this, read operations are required when there is a frame in the session.
        //
        // Another solution to avoid this problem is to let the session and stream share the state.
        // In the rust implementation, at least the following three states are required:
        // 1. writeable_wake
        // 2. send_window
        // 3. state
        //
        // When the session receives a window update frame, it can update the state of the stream.
        // In the implementation here, we try not to share state between the session and the stream.
        // If a read task is parked (`readable_wake` set), the read path owns
        // `frame_receiver`'s waker: it will process window updates and wake us
        // via `writeable_wake`. Use the non-blocking drain so we don't
        // overwrite the reader's waker. Otherwise (no read task, e.g. one-way
        // bulk send), register this task's waker with the blocking drain so a
        // later window update can wake us directly.
        let recv_res = if self.readable_wake.is_some() {
            self.recv_frames_wake()
        } else {
            self.recv_frames_wake_blocking(cx)
        };
        if let Err(Error::UnexpectedFlag | Error::RecvWindowExceeded | Error::InvalidMsgType) =
            recv_res
        {
            // read flag error or read data error
            self.send_go_away();
        }

        match self.state {
            StreamState::Reset => return Poll::Ready(Err(io::ErrorKind::BrokenPipe.into())),
            StreamState::LocalClosing | StreamState::Closed => {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "The local is closed and data cannot be written.",
                )));
            }
            _ => (),
        }

        if self.send_window == 0 {
            // register writer context waker
            // when write buf become empty, it can wake the upper layer to write the message again
            self.writeable_wake = Some(cx.waker().clone());
            // A window update can arrive after the first non-blocking drain
            // but before the writer is parked. Re-drain after registering the
            // writer so that update cannot be left queued with no live poller.
            let reader_is_current_task = self
                .readable_wake
                .as_ref()
                .is_some_and(|waker| waker.will_wake(cx.waker()));
            let recv_res = if reader_is_current_task || self.readable_wake.is_none() {
                self.recv_frames_wake_blocking(cx)
            } else {
                self.recv_frames_wake()
            };
            if let Err(Error::UnexpectedFlag | Error::RecvWindowExceeded | Error::InvalidMsgType) =
                recv_res
            {
                self.send_go_away();
            }
            match self.state {
                StreamState::Reset => return Poll::Ready(Err(io::ErrorKind::BrokenPipe.into())),
                StreamState::LocalClosing | StreamState::Closed => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "The local is closed and data cannot be written.",
                    )));
                }
                _ => (),
            }
            if self.send_window > 0 {
                self.writeable_wake = None;
            } else {
                return Poll::Pending;
            }
        }
        // Allow n = 0, send an empty frame to remote
        let n = ::std::cmp::min(self.send_window as usize, buf.len());
        trace!(
            "stream-hanlde({}) poll_write self.send_window={}, buf.len={}, n={}",
            self.id,
            self.send_window,
            buf.len(),
            n,
        );
        let data = &buf[0..n];
        match self.send_data(data) {
            Ok(_) => {
                self.send_window -= n as u32;

                Poll::Ready(Ok(n))
            }
            Err(Error::WouldBlock) => Poll::Pending,
            _ => Poll::Ready(Err(io::ErrorKind::BrokenPipe.into())),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, _cx: &mut Context) -> Poll<io::Result<()>> {
        debug!("[{}] StreamHandle.shutdown()", self.id);
        match self.close() {
            Err(_) => Poll::Ready(Err(io::ErrorKind::BrokenPipe.into())),
            Ok(()) => Poll::Ready(Ok(())),
        }
    }
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        if !self.unbound_event_sender.is_closed() && self.state != StreamState::Closed {
            match self.state {
                // LocalClosing means that local have sent Fin to the remote and waiting for a response.
                StreamState::LocalClosing | StreamState::Reset => (),
                // if not, we should send Rst first
                StreamState::Established
                | StreamState::Init
                | StreamState::RemoteClosing
                | StreamState::SynReceived
                | StreamState::SynSent => {
                    let mut flags = self.get_flags();
                    flags.add(Flag::Rst);
                    let frame = Frame::new_window_update(flags, self.id, 0);
                    let rst_event = StreamEvent::Frame(frame);

                    // Always successful unless the session is dropped
                    let _ignore = self.unbound_event_sender.unbounded_send(rst_event);
                }
                StreamState::Closed => unreachable!(
                    "yamux stream {} entered Drop with Closed state despite the state guard",
                    self.id
                ),
            }

            let event = StreamEvent::Closed(self.id);
            let _ignore = self.unbound_event_sender.unbounded_send(event);
        }
    }
}

// Stream event
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum StreamEvent {
    Frame(Frame),
    Closed(StreamId),
    // Only use on protocol error
    GoAway,
}

/// The stream state
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum StreamState {
    /// Just created
    Init,
    /// We sent a Syn message
    SynSent,
    /// We received a Syn message
    SynReceived,
    /// Stream established
    Established,
    /// We closed the stream
    LocalClosing,
    /// Remote closed the stream
    RemoteClosing,
    /// Both side of the stream closed
    Closed,
    /// Stream rejected by remote
    Reset,
}

#[cfg(test)]
mod test {
    use super::{
        StreamEvent, StreamHandle, StreamState, receive_credit_delta,
        should_shrink_read_buffer,
    };
    use crate::{
        config::INITIAL_STREAM_WINDOW,
        frame::{Flag, Flags, Frame, Type},
        session::rt,
    };
    use bytes::BytesMut;
    use futures::{
        FutureExt, SinkExt, StreamExt,
        channel::mpsc::{channel, unbounded},
        future::join_all,
        task::{ArcWake, waker_ref},
    };
    use rand::{Rng, SeedableRng, rngs::StdRng};
    use std::{
        io::ErrorKind,
        pin::Pin,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    };
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

    #[derive(Default)]
    struct FlagWaker(AtomicBool);
    impl ArcWake for FlagWaker {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            arc_self.0.store(true, Ordering::SeqCst);
        }
    }
    impl FlagWaker {
        fn woken(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
    }

    #[test]
    fn test_drop() {
        let rt = rt();
        rt.block_on(async {
            let (_frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, mut unbound_receiver) = unbounded();
            let stream = StreamHandle::new(
                0,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );

            drop(stream);
            let event = unbound_receiver.next().await.unwrap();
            match event {
                StreamEvent::Frame(frame) => assert!(frame.flags().contains(Flag::Rst)),
                _ => panic!("must be a frame msg contain RST"),
            }
            let event = unbound_receiver.next().await.unwrap();
            match event {
                StreamEvent::Closed(_) => (),
                _ => panic!("must be state closed"),
            }
        });
    }

    #[test]
    fn concurrent_stream_shutdowns_emit_one_fin_and_close_each_stream() {
        rt().block_on(async {
            let (event_sender, mut events) = unbounded();
            let mut streams = (0..64)
                .map(|id| {
                    let (_frame_sender, frame_receiver) = channel(2);
                    StreamHandle::new(
                        id,
                        event_sender.clone(),
                        frame_receiver,
                        StreamState::Init,
                        INITIAL_STREAM_WINDOW,
                    )
                })
                .collect::<Vec<_>>();

            let results = join_all(streams.iter_mut().map(|stream| stream.shutdown())).await;
            assert!(results.iter().all(Result::is_ok));
            drop(streams);

            let mut fins = 0;
            let mut closed = 0;
            while let Ok(event) = events.try_recv() {
                match event {
                    StreamEvent::Frame(frame) => {
                        assert!(frame.flags().contains(Flag::Fin));
                        fins += 1;
                    }
                    StreamEvent::Closed(_) => closed += 1,
                    StreamEvent::GoAway => panic!("unexpected session close event"),
                }
            }
            assert_eq!(fins, 64);
            assert_eq!(closed, 64);
        });
    }

    #[test]
    fn test_drop_with_state_reset() {
        let rt = rt();
        rt.block_on(async {
            let (mut frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, mut unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                0,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );

            let mut flags = Flags::from(Flag::Syn);
            flags.add(Flag::Rst);
            let frame = Frame::new_window_update(flags, 0, 0);
            frame_sender.send(frame).await.unwrap();
            let mut b = [0; 1024];

            // try poll stream handle, then it will recv RST frame and set self state to reset
            assert_eq!(
                stream.read(&mut b).await.unwrap_err().kind(),
                ErrorKind::ConnectionReset
            );

            assert_eq!(stream.state, StreamState::Reset);

            drop(stream);

            let event = unbound_receiver.next().await.unwrap();
            match event {
                StreamEvent::Closed(_) => (),
                _ => panic!("must be state closed"),
            }
        });
    }

    #[test]
    fn reset_delivers_queued_data_then_reports_reset() {
        rt().block_on(async {
            let (mut frame_sender, frame_receiver) = channel(4);
            let (unbound_sender, _unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                7,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );

            frame_sender
                .send(Frame::new_data(
                    Flags::default(),
                    7,
                    BytesMut::from(&b"queued before reset"[..]),
                ))
                .await
                .unwrap();
            let mut rst = Flags::from(Flag::Rst);
            rst.add(Flag::Ack);
            frame_sender
                .send(Frame::new_data(
                    rst,
                    7,
                    BytesMut::from(&b"reset payload"[..]),
                ))
                .await
                .unwrap();

            // Bytes legitimately received before the reset are still delivered;
            // only the payload carried by the DATA+RST frame itself is discarded.
            let mut byte = [0; 32];
            let n = stream.read(&mut byte).await.unwrap();
            assert_eq!(&byte[..n], b"queued before reset");
            // Once the queue drains, the reset surfaces as an error.
            assert_eq!(
                stream.read(&mut byte).await.unwrap_err().kind(),
                ErrorKind::ConnectionReset
            );
            assert!(stream.read_buf.is_empty());
            assert_eq!(stream.state(), StreamState::Reset);
        });
    }

    #[test]
    fn test_drop_with_state_local_close() {
        let rt = rt();
        rt.block_on(async {
            let (_frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, mut unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                0,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );

            let _ignore = stream.shutdown().await;

            let event = unbound_receiver.next().await.unwrap();
            match event {
                StreamEvent::Frame(frame) => {
                    assert!(frame.flags().contains(Flag::Fin));
                    assert_eq!(frame.ty(), Type::WindowUpdate);
                }
                _ => panic!("must be fin window update"),
            }

            drop(stream);
            let event = unbound_receiver.next().await.unwrap();
            match event {
                StreamEvent::Closed(_) => (),
                _ => panic!("must be state closed"),
            }
        });
    }

    #[test]
    fn test_data_large_than_recv_window() {
        let rt = rt();
        rt.block_on(async {
            let (mut frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, mut unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                0,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );

            stream.recv_window = 2;

            let flags = Flags::from(Flag::Syn);
            let frame = Frame::new_data(flags, 0, BytesMut::from("1234"));
            frame_sender.send(frame).await.unwrap();
            let mut b = [0; 1024];

            // try poll stream handle, then it will recv data frame and return Err
            assert_eq!(
                stream.read(&mut b).await.unwrap_err().kind(),
                ErrorKind::InvalidData
            );

            let event = unbound_receiver.next().await.unwrap();
            match event {
                StreamEvent::GoAway => (),
                _ => panic!("must be go away"),
            }
        });
    }

    #[test]
    fn window_update_threshold_sends_at_half_window() {
        let rt = rt();
        rt.block_on(async {
            let (_frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, mut unbound_receiver) = unbounded();
            let max_window = INITIAL_STREAM_WINDOW * 2;
            let mut stream = StreamHandle::new(
                1,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                max_window,
            );
            stream.state = StreamState::Established;
            stream.recv_window = INITIAL_STREAM_WINDOW;

            // One queued byte leaves the credit increase just below the half-window threshold.
            stream.read_buf.push(BytesMut::from(&b"x"[..]));
            stream.send_window_update().unwrap();
            assert!(unbound_receiver.next().now_or_never().is_none());

            // Once the buffered byte is consumed, the increase reaches exactly half the ceiling.
            stream.read_buf.clear();
            stream.send_window_update().unwrap();
            match unbound_receiver.next().await.unwrap() {
                StreamEvent::Frame(frame) => {
                    assert_eq!(frame.ty(), Type::WindowUpdate);
                    assert_eq!(frame.length(), INITIAL_STREAM_WINDOW);
                }
                _ => panic!("half-window credit must emit a window update"),
            }
            assert_eq!(stream.recv_window(), max_window);
        });
    }

    #[test]
    fn odd_window_update_threshold_rounds_up_and_respects_ceiling() {
        let rt = rt();
        rt.block_on(async {
            let (_frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, mut unbound_receiver) = unbounded();
            let max_window = 101;
            let mut stream = StreamHandle::new(
                1,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                max_window,
            );
            stream.state = StreamState::Established;
            stream.recv_window = 50;

            // One byte of buffering leaves 50 bytes of credit, below
            // ceil(101 / 2) = 51.
            stream.read_buf.push(BytesMut::from(&b"x"[..]));
            stream.send_window_update().unwrap();
            assert!(unbound_receiver.next().now_or_never().is_none());
            assert_eq!(stream.recv_window(), 50);

            // Consuming the byte makes the available credit 51; the update
            // reaches, but never exceeds, the configured maximum.
            stream.read_buf.clear();
            stream.send_window_update().unwrap();
            match unbound_receiver.next().await.unwrap() {
                StreamEvent::Frame(frame) => assert_eq!(frame.length(), 51),
                _ => panic!("half-window credit must emit a window update"),
            }
            assert_eq!(stream.recv_window(), max_window);

            stream.send_window_update().unwrap();
            assert!(unbound_receiver.next().now_or_never().is_none());
            assert_eq!(stream.recv_window(), max_window);
        });
    }

    #[test]
    fn exhausted_receive_window_is_restored_after_buffer_is_consumed() {
        let rt = rt();
        rt.block_on(async {
            let (_frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, mut unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                1,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );
            stream.state = StreamState::Established;
            stream.recv_window = 0;
            stream
                .read_buf
                .push(BytesMut::zeroed(INITIAL_STREAM_WINDOW as usize));

            // Buffered bytes still occupy the entire receive budget, so no
            // credit may be re-advertised while the application has not read them.
            stream.send_window_update().unwrap();
            assert_eq!(stream.recv_window(), 0);
            assert!(unbound_receiver.next().now_or_never().is_none());

            stream.read_buf.clear();
            stream.send_window_update().unwrap();
            match unbound_receiver.next().await.unwrap() {
                StreamEvent::Frame(frame) => {
                    assert_eq!(frame.ty(), Type::WindowUpdate);
                    assert_eq!(frame.length(), INITIAL_STREAM_WINDOW);
                }
                _ => panic!("consuming the buffer must advertise receive credit"),
            }
            assert_eq!(stream.recv_window(), INITIAL_STREAM_WINDOW);
        });
    }

    #[test]
    fn zero_send_window_requires_positive_update_to_resume_writer() {
        let rt = rt();
        rt.block_on(async {
            let (_frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, _unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                1,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );
            stream.state = StreamState::Established;
            stream.send_window = 0;

            let blocked = stream.write(b"x").now_or_never();
            assert!(
                blocked.is_none(),
                "an exhausted window must backpressure writes"
            );

            stream
                .handle_window_update(&Frame::new_window_update(Flags::default(), 1, 0))
                .unwrap();
            assert_eq!(stream.send_window(), 0);
            let blocked = stream.write(b"x").now_or_never();
            assert!(blocked.is_none(), "zero credit must not resume the writer");

            stream
                .handle_window_update(&Frame::new_window_update(Flags::default(), 1, 1))
                .unwrap();
            assert_eq!(stream.send_window(), 1);
            assert_eq!(stream.write(b"x").await.unwrap(), 1);
            assert_eq!(stream.send_window(), 0);
        });
    }

    #[test]
    fn invalid_receive_credit_does_not_emit_wrapped_window_update() {
        let rt = rt();
        rt.block_on(async {
            let (_frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, mut unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                1,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );
            stream.state = StreamState::Established;
            stream.recv_window = INITIAL_STREAM_WINDOW + 1;

            assert_eq!(
                stream.send_window_update(),
                Err(crate::Error::InvalidMsgType)
            );
            assert_eq!(stream.recv_window(), INITIAL_STREAM_WINDOW + 1);
            assert!(unbound_receiver.next().now_or_never().is_none());
        });
    }

    #[test]
    fn receive_credit_accounting_checks_conversion_sum_and_available_credit() {
        assert_eq!(
            receive_credit_delta(100, 40, [20usize, 10].into_iter()),
            Ok(30)
        );
        assert_eq!(
            receive_credit_delta(100, 40, [usize::MAX].into_iter()),
            Err(crate::Error::InvalidMsgType)
        );
        assert_eq!(
            receive_credit_delta(100, 40, [u32::MAX as usize, 1].into_iter()),
            Err(crate::Error::InvalidMsgType)
        );
        assert_eq!(
            receive_credit_delta(100, 40, [61usize].into_iter()),
            Err(crate::Error::InvalidMsgType)
        );
        assert_eq!(
            receive_credit_delta(100, 101, [].into_iter()),
            Err(crate::Error::InvalidMsgType)
        );
    }

    #[test]
    fn receive_credit_accounting_handles_u32_window_ceiling() {
        assert_eq!(receive_credit_delta(u32::MAX, u32::MAX, [].into_iter()), Ok(0));
        assert_eq!(
            receive_credit_delta(u32::MAX, u32::MAX - 1, [].into_iter()),
            Ok(1)
        );
        assert_eq!(
            receive_credit_delta(u32::MAX, u32::MAX - 2, [1usize].into_iter()),
            Ok(1)
        );
        assert_eq!(
            receive_credit_delta(u32::MAX, u32::MAX, [1usize].into_iter()),
            Err(crate::Error::InvalidMsgType)
        );
    }

    #[test]
    fn randomized_receive_credit_updates_preserve_window_invariant() {
        let rt = rt();
        rt.block_on(async {
            let (_frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, mut unbound_receiver) = unbounded();
            let max_window = INITIAL_STREAM_WINDOW * 2;
            let mut stream = StreamHandle::new(
                7,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                max_window,
            );
            stream.state = StreamState::Established;
            let mut rng = StdRng::seed_from_u64(0xc4ed_17);

            for _ in 0..1000 {
                let buffered: usize = stream.read_buf.iter().map(BytesMut::len).sum();
                assert!(u64::from(stream.recv_window) + buffered as u64 <= u64::from(max_window));

                if rng.gen_bool(0.6) && stream.recv_window > 0 {
                    let len = rng.gen_range(1..=stream.recv_window.min(512));
                    let frame = Frame::new_data(
                        Flags::default(),
                        7,
                        BytesMut::from(vec![0x5a; len as usize].as_slice()),
                    );
                    stream.handle_data(frame).unwrap();
                } else {
                    // Model application consumption; credit is replenished below.
                    stream.read_buf.clear();
                }

                let buffered: usize = stream.read_buf.iter().map(BytesMut::len).sum();
                let expected_delta = max_window - buffered as u32 - stream.recv_window;
                let should_send = expected_delta >= max_window / 2;
                let recv_before = stream.recv_window;
                stream.send_window_update().unwrap();

                if should_send {
                    match unbound_receiver.next().await.unwrap() {
                        StreamEvent::Frame(frame) => {
                            assert_eq!(frame.ty(), Type::WindowUpdate);
                            assert_eq!(frame.length(), expected_delta);
                        }
                        _ => panic!("expected window update frame"),
                    }
                    assert_eq!(stream.recv_window, recv_before + expected_delta);
                } else {
                    assert!(unbound_receiver.next().now_or_never().is_none());
                    assert_eq!(stream.recv_window, recv_before);
                }

                let buffered: usize = stream.read_buf.iter().map(BytesMut::len).sum();
                assert!(u64::from(stream.recv_window) + buffered as u64 <= u64::from(max_window));
            }
        });
    }

    #[test]
    fn stream_flags_force_window_update_below_threshold() {
        let rt = rt();
        rt.block_on(async {
            let (_frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, mut unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                1,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW * 2,
            );
            // Init emits SYN even though the replenished credit is below threshold.
            stream.recv_window = INITIAL_STREAM_WINDOW * 2 - 1;
            stream.send_window_update().unwrap();
            match unbound_receiver.next().await.unwrap() {
                StreamEvent::Frame(frame) => {
                    assert!(frame.flags().contains(Flag::Syn));
                    assert_eq!(frame.length(), 1);
                }
                _ => panic!("initial window update must emit a frame"),
            }
            assert_eq!(stream.recv_window(), INITIAL_STREAM_WINDOW * 2);
        });
    }

    #[test]
    fn failed_window_update_send_preserves_receive_credit() {
        let rt = rt();
        rt.block_on(async {
            let (_frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, unbound_receiver) = unbounded();
            drop(unbound_receiver);
            let mut stream = StreamHandle::new(
                1,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW * 2,
            );
            stream.state = StreamState::Established;
            stream.recv_window = INITIAL_STREAM_WINDOW;
            assert_eq!(
                stream.send_window_update(),
                Err(crate::Error::SessionShutdown)
            );
            assert_eq!(stream.recv_window(), INITIAL_STREAM_WINDOW);
        });
    }

    #[test]
    fn overflowing_window_update_preserves_send_credit() {
        let rt = rt();
        rt.block_on(async {
            let (_frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, _unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                1,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );
            stream.state = StreamState::Established;
            stream.send_window = u32::MAX - 3;
            let update = Frame::new_window_update(Flags::default(), 1, 4);

            assert_eq!(
                stream.handle_window_update(&update),
                Err(crate::Error::InvalidMsgType)
            );
            assert_eq!(stream.send_window(), u32::MAX - 3);
        });
    }

    #[test]
    fn write_after_reset_is_broken_pipe() {
        let rt = rt();
        rt.block_on(async {
            let (_frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, _unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                0,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );
            stream.state = StreamState::Reset;
            assert_eq!(
                stream.write(b"x").await.unwrap_err().kind(),
                ErrorKind::BrokenPipe
            );
        });
    }

    #[test]
    fn write_after_local_close_is_broken_pipe() {
        let rt = rt();
        rt.block_on(async {
            let (_frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, _unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                0,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );
            stream.state = StreamState::LocalClosing;
            assert_eq!(
                stream.write(b"x").await.unwrap_err().kind(),
                ErrorKind::BrokenPipe
            );
        });
    }

    #[test]
    fn peek_reports_invalid_frame_as_invalid_data() {
        let rt = rt();
        rt.block_on(async {
            let (mut frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, mut unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                0,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );
            stream.recv_window = 0;
            // A payload larger than the receive window is rejected by recv_frames.
            frame_sender
                .send(Frame::new_data(
                    Flags::from(Flag::Syn),
                    0,
                    BytesMut::from("x"),
                ))
                .await
                .unwrap();
            assert_eq!(
                stream.peek(&mut [0; 1]).await.unwrap_err().kind(),
                ErrorKind::InvalidData
            );
            assert!(matches!(
                unbound_receiver.next().await,
                Some(StreamEvent::GoAway)
            ));
        });
    }

    // https://github.com/nervosnetwork/tentacle/issues/297
    //
    // As you can see from the description, the real cause of the problem
    // is that the two channels cannot guarantee the consistency of the sending
    // order, that is, the order of the message to start the stream and the
    // message to send data is reversed, causing the remote end to receive
    // an unowned message , Silently discarded, causing the problem that
    // the protocol cannot be opened
    #[test]
    fn test_open_stream_with_data() {
        let rt = rt();
        rt.block_on(async {
            let (_frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, mut unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                0,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );

            let data = [0; 8];

            stream.send_window_update().unwrap();
            stream.write_all(&data).await.unwrap();

            let event = unbound_receiver.next().await.unwrap();
            match event {
                StreamEvent::Frame(frame) => assert!(frame.ty() == Type::WindowUpdate),
                _ => panic!("must be a window update msg"),
            }

            let event = unbound_receiver.next().await.unwrap();
            match event {
                StreamEvent::Frame(frame) => assert!(frame.ty() == Type::Data),
                _ => panic!("must be a frame msg"),
            }
        });
    }

    #[test]
    fn test_read_with_half_close() {
        let rt = rt();
        rt.block_on(async {
            let (mut frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, _unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                0,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );

            stream.shutdown().await.unwrap();

            assert_eq!(stream.state, StreamState::LocalClosing);

            let flags = Flags::from(Flag::Syn);
            let frame = Frame::new_data(flags, 0, BytesMut::from("1234"));
            frame_sender.send(frame).await.unwrap();
            let mut b = [0; 1024];

            assert_eq!(stream.read(&mut b).await.unwrap(), 4);
            assert_eq!(&b[..4], b"1234");

            assert_eq!(stream.state, StreamState::LocalClosing);
        });
    }

    #[test]
    fn test_write_with_half_close() {
        let rt = rt();
        rt.block_on(async {
            let (mut frame_sender, frame_receiver) = channel(2);
            let (unbound_sender, mut unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                0,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );

            let flags = Flags::from(Flag::Fin);
            let frame = Frame::new_window_update(flags, 0, 0);
            frame_sender.send(frame).await.unwrap();
            let mut b = [0; 1024];

            assert_eq!(stream.read(&mut b).await.unwrap(), 0);
            assert_eq!(stream.state, StreamState::RemoteClosing);

            const TEXT: &[u8] = b"testtext";

            let jh = tokio::spawn(tokio::time::timeout(std::time::Duration::from_secs(4), async move {
                loop {
                    match unbound_receiver.try_recv() {
                        Ok(ref event) if matches!(event, StreamEvent::Frame(frame) if frame.length() == TEXT.len() as u32) => break,
                        Err(_) => (),
                        _ => panic!("must be frame with written text"),
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
            }));

            stream.write_all(TEXT).await.unwrap();

            jh.await.unwrap().expect("not tiemout");

            assert_eq!(stream.state, StreamState::RemoteClosing);
        });
    }

    #[test]
    fn test_half_close_preserves_reverse_direction_until_peer_fin() {
        let rt = rt();
        rt.block_on(async {
            let (mut frame_sender, frame_receiver) = channel(4);
            let (unbound_sender, mut unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                7,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );

            stream.write_all(b"request").await.unwrap();
            let StreamEvent::Frame(request) = unbound_receiver.next().await.unwrap() else {
                panic!("request must produce a data frame");
            };
            assert_eq!(request.ty(), Type::Data);
            assert_eq!(request.flags(), Flags::from(Flag::Syn));
            let (_, body) = request.into_parts();
            assert_eq!(&body.unwrap()[..], b"request");

            stream.shutdown().await.unwrap();
            assert_eq!(stream.state, StreamState::LocalClosing);
            let StreamEvent::Frame(fin) = unbound_receiver.next().await.unwrap() else {
                panic!("shutdown must produce a FIN frame");
            };
            assert!(fin.flags().contains(Flag::Fin));

            // A peer may continue sending its response after our write half closes.
            frame_sender
                .send(Frame::new_data(
                    Flags::from(Flag::Ack),
                    7,
                    BytesMut::from("response"),
                ))
                .await
                .unwrap();
            let mut response = [0; 16];
            let n = stream.read(&mut response).await.unwrap();
            assert_eq!(&response[..n], b"response");
            assert_eq!(stream.state, StreamState::LocalClosing);

            frame_sender
                .send(Frame::new_window_update(Flags::from(Flag::Fin), 7, 0))
                .await
                .unwrap();
            assert_eq!(stream.read(&mut response).await.unwrap(), 0);
            assert_eq!(stream.state, StreamState::Closed);
        });
    }

    #[test]
    fn test_frame_read_more_than_one() {
        let rt = rt();
        rt.block_on(async {
            let (mut frame_sender, frame_receiver) = channel(3);
            let (unbound_sender, _unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                0,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );

            let flags = Flags::from(Flag::Syn);
            let frame = Frame::new_data(flags, 0, BytesMut::from("1234"));
            frame_sender.send(frame).await.unwrap();
            let flags = Flags::from(Flag::Syn);
            let frame = Frame::new_data(flags, 0, BytesMut::default());
            frame_sender.send(frame).await.unwrap();
            let flags = Flags::from(Flag::Syn);
            let frame = Frame::new_data(flags, 0, BytesMut::from("5678"));
            frame_sender.send(frame).await.unwrap();
            let mut b = [0; 2];

            assert_eq!(stream.read(&mut b).await.unwrap(), 2);
            assert_eq!(&b[..2], b"12");
            assert_eq!(stream.read_buf.len(), 2);
            assert_eq!(stream.read_buf.capacity(), 4);

            assert_eq!(stream.read(&mut b).await.unwrap(), 2);
            assert_eq!(&b[..2], b"34");
            assert_eq!(stream.read_buf.len(), 1);
            // Drain does not shrink the capacity
            assert_eq!(stream.read_buf.capacity(), 4);

            let flags = Flags::from(Flag::Syn);
            let frame = Frame::new_data(flags, 0, BytesMut::default());
            frame_sender.send(frame).await.unwrap();
            let flags = Flags::from(Flag::Syn);
            let frame = Frame::new_data(flags, 0, BytesMut::from("1234"));
            frame_sender.send(frame).await.unwrap();
            let mut c = [0; 5];

            assert_eq!(stream.read(&mut c).await.unwrap(), 5);
            assert_eq!(&c[..5], b"56781");
            assert_eq!(stream.read_buf.len(), 1);
            assert_eq!(stream.read_buf.capacity(), 4);

            assert_eq!(stream.read(&mut b).await.unwrap(), 2);
            assert_eq!(&b[..2], b"23");
            assert_eq!(stream.read_buf.len(), 1);
            assert_eq!(stream.read_buf.capacity(), 4);
        });
    }

    #[test]
    fn read_buffer_releases_excess_capacity_after_drain() {
        let rt = rt();
        rt.block_on(async {
            let (_frame_sender, frame_receiver) = channel(1);
            let (unbound_sender, _unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                0,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );
            stream.read_buf = Vec::with_capacity(128);
            stream.read_buf.push(BytesMut::from(&b"payload"[..]));

            let mut received = [0; 7];
            assert_eq!(stream.read(&mut received).await.unwrap(), 7);
            assert_eq!(&received, b"payload");
            assert!(stream.read_buf.is_empty());
            assert!(stream.read_buf.capacity() <= 24);
        });
    }

    #[test]
    fn read_buffer_shrink_policy_preserves_small_and_dense_queues() {
        assert!(!should_shrink_read_buffer(24, 0));
        assert!(!should_shrink_read_buffer(25, 5));
        assert!(should_shrink_read_buffer(25, 4));
        assert!(!should_shrink_read_buffer(128, 30));
        assert!(should_shrink_read_buffer(128, 24));
    }

    // Regression test for:
    //   `poll_write` calling `recv_frames(write_cx)` which polls `frame_receiver`
    //   with the **write** task's Context and thereby overwrites the read task's waker
    //   that was stored there by `poll_read`.  Once the write_waker is stale (write
    //   task finished), any incoming data frame wakes nobody → read side hangs.
    //
    // This test is fully deterministic: it uses custom flag-wakers and manually drives
    // `poll_read` / `poll_write`, so there is no reliance on tokio scheduler ordering.
    //
    // Failure scenario (old buggy code):
    //   1. poll_read(read_cx)  → recv_frames(read_cx)  → read_waker  stored in frame_receiver
    //   2. poll_write(write_cx)→ recv_frames(write_cx) → write_waker stored in frame_receiver
    //                                                     (OVERWRITES read_waker)
    //   3. Data frame injected → frame_receiver wakes write_waker (stale / already done)
    //      → read_waker is NEVER notified → read side hangs forever.
    //
    // With the fix (try_recv_frames / try_next):
    //   Step 2 does NOT touch frame_receiver's stored waker.
    //   Step 3 correctly wakes read_waker.
    #[test]
    fn test_write_side_does_not_overwrite_read_waker() {
        let rt = rt();
        rt.block_on(async {
            let (mut frame_sender, frame_receiver) = channel(128);
            let (unbound_sender, _unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                1,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );

            let read_fw = Arc::new(FlagWaker::default());
            let write_fw = Arc::new(FlagWaker::default());
            let read_waker_ref = waker_ref(&read_fw);
            let write_waker_ref = waker_ref(&write_fw);
            let mut read_cx = Context::from_waker(&read_waker_ref);
            let mut write_cx = Context::from_waker(&write_waker_ref);

            // Step 1: poll_read → parks → read_waker registered in frame_receiver.
            let mut buf = vec![0u8; 32];
            let mut rbuf = ReadBuf::new(&mut buf);
            let r = Pin::new(&mut stream).poll_read(&mut read_cx, &mut rbuf);
            assert!(
                r.is_pending(),
                "poll_read must return Pending (no data yet)"
            );

            // Step 2: poll_write (send_window > 0, succeeds immediately).
            //   OLD buggy code:  recv_frames_wake(write_cx) → recv_frames(write_cx)
            //                    → frame_receiver.poll_next(write_cx) → OVERWRITES read_waker
            //                      with write_waker.
            //   NEW fixed code:  recv_frames_wake(_cx) → try_recv_frames() → try_next()
            //                    → does NOT touch frame_receiver's stored waker at all.
            let r = Pin::new(&mut stream).poll_write(&mut write_cx, b"ping");
            assert!(
                matches!(r, Poll::Ready(Ok(4))),
                "poll_write must succeed (send_window has capacity)"
            );

            // Step 3: inject an incoming data frame.
            // The mpsc channel calls wake() synchronously on the stored waker when an
            // item is enqueued while the receiver is waiting.
            //
            // OLD (bug):  write_waker was stored last → write_fw.woken() == true,
            //             read_fw.woken()  == false  → read side would hang.
            // NEW (fix):  read_waker  was stored last → read_fw.woken()  == true.
            let frame = Frame::new_data(Flags::from(Flag::Syn), 1, BytesMut::from("data"));
            frame_sender.send(frame).await.unwrap();

            assert!(
                read_fw.woken(),
                "BUG REPRODUCED: read_waker was overwritten by write side; \
                 incoming data frame woke write_waker instead of read_waker. \
                 The read side would hang forever."
            );
            assert!(
                !write_fw.woken(),
                "write_waker must NOT be stored in frame_receiver \
                 (only the read side should register its waker there)"
            );
        });
    }

    // Regression test: when send_window == 0 (write side blocked), a window-update
    // frame from the remote must travel via the READ path—not the write path—to
    // unblock the write side.
    //
    // Correct flow (with fix):
    //   window_update arrives → frame_receiver wakes read_waker (read path owns it)
    //   → poll_read processes handle_window_update → send_window increases
    //   → writeable_wake.wake() → write_waker notified → write side can proceed.
    //
    // Buggy flow (old code):
    //   recv_frames(write_cx) stored write_waker in frame_receiver, overwriting read_waker.
    //   window_update arrives → write_waker notified → write task itself drains the frame,
    //   which accidentally works for the write side, BUT the read task's waker is now gone.
    //   Any subsequent DATA frame would silently wake the stale write_waker → read hangs.
    //
    // This test is fully deterministic via custom flag-wakers and manual polling.
    #[test]
    fn test_window_update_wakes_write_via_read_path() {
        let rt = rt();
        rt.block_on(async {
            let (mut frame_sender, frame_receiver) = channel(128);
            let (unbound_sender, _unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                1,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );

            // Exhaust the send window so poll_write will park.
            stream.send_window = 0;

            let read_fw = Arc::new(FlagWaker::default());
            let write_fw = Arc::new(FlagWaker::default());
            let read_waker_ref = waker_ref(&read_fw);
            let write_waker_ref = waker_ref(&write_fw);
            let mut read_cx = Context::from_waker(&read_waker_ref);
            let mut write_cx = Context::from_waker(&write_waker_ref);

            // Step 1: poll_read → parks → read_waker registered in frame_receiver,
            //         readable_wake = read_waker.
            let mut buf = vec![0u8; 32];
            let mut rbuf = ReadBuf::new(&mut buf);
            assert!(
                Pin::new(&mut stream)
                    .poll_read(&mut read_cx, &mut rbuf)
                    .is_pending()
            );

            // Step 2: poll_write → send_window == 0 → parks.
            //   OLD: recv_frames(write_cx) first OVERWRITES frame_receiver's waker with
            //        write_waker.  Then send_window==0 → writeable_wake = write_waker.
            //   NEW: try_recv_frames() does NOT touch frame_receiver waker.
            //        send_window==0 → writeable_wake = write_waker.
            assert!(
                Pin::new(&mut stream)
                    .poll_write(&mut write_cx, b"ping")
                    .is_pending()
            );

            // Step 3: inject a window-update frame (simulates remote granting more window).
            // This synchronously wakes whoever is registered in frame_receiver.
            //   OLD (bug): write_waker → write_fw.woken() == true BEFORE we poll_read.
            //              But frame_receiver now holds write_waker (stale once write is done).
            //   NEW (fix): read_waker → read_fw.woken() == true.
            let wu = Frame::new_window_update(Flags::default(), 1, 65535);
            frame_sender.send(wu).await.unwrap();

            // With the fix, the read_waker must be the one notified.
            assert!(
                read_fw.woken(),
                "BUG: read_waker was overwritten; window-update woke write_waker instead. \
                 The read path cannot process the window-update → write side stays stuck."
            );
            assert!(
                !write_fw.woken(),
                "write_waker must not be in frame_receiver; it should only be in writeable_wake"
            );

            // Step 4: simulate the read task re-polling after being woken.
            // poll_read processes the window-update frame via handle_window_update,
            // which increases send_window and calls writeable_wake.wake().
            let mut rbuf2 = ReadBuf::new(&mut buf);
            // poll_read will drain the window-update frame and internally call
            // writeable_wake.wake(), which notifies write_fw.
            // (The window-update has no data so read returns Pending again.)
            let _ignore = Pin::new(&mut stream).poll_read(&mut read_cx, &mut rbuf2);

            // After handle_window_update → writeable_wake.wake(), the write task
            // (write_waker) must now be notified so it can retry and succeed.
            assert!(
                write_fw.woken(),
                "write_waker must be notified via writeable_wake after \
                 the read path processes the window-update frame"
            );

            // Step 5: poll_write again now that send_window > 0.
            let r = Pin::new(&mut stream).poll_write(&mut write_cx, b"ping");
            assert!(
                matches!(r, Poll::Ready(Ok(4))),
                "poll_write must now succeed after window was restored"
            );
        });
    }

    // Put a WINDOW_UPDATE into the channel exactly when poll_write registers
    // its write waker: after the initial drain and before the re-drain.
    #[test]
    fn test_poll_write_redrains_window_update_before_parking() {
        let rt = rt();
        rt.block_on(async {
            let (frame_sender, frame_receiver) = channel(128);
            let (unbound_sender, _unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                1,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );
            stream.send_window = 0;

            // Keep readable_wake set so poll_write uses the non-blocking path,
            // preserving the reader's registration in frame_receiver.
            let read_fw = Arc::new(FlagWaker::default());
            let read_waker_ref = waker_ref(&read_fw);
            let mut read_cx = Context::from_waker(&read_waker_ref);
            let mut read_storage = [0; 1];
            let mut read_buf = ReadBuf::new(&mut read_storage);
            assert!(
                Pin::new(&mut stream)
                    .poll_read(&mut read_cx, &mut read_buf)
                    .is_pending()
            );

            struct EnqueueOnClone(Mutex<Option<futures::channel::mpsc::Sender<Frame>>>);
            #[allow(unsafe_op_in_unsafe_fn)]
            unsafe fn clone_waker(data: *const ()) -> RawWaker {
                let arc = Arc::<EnqueueOnClone>::from_raw(data.cast());
                if let Some(mut sender) = arc.0.lock().unwrap().take() {
                    // The first clone is the writeable_wake registration, which
                    // occurs between the two drains.
                    let _ = sender.try_send(Frame::new_window_update(Flags::default(), 1, 65535));
                }
                let cloned = arc.clone();
                let _ = Arc::into_raw(arc);
                RawWaker::new(Arc::into_raw(cloned).cast(), &VTABLE)
            }
            #[allow(unsafe_op_in_unsafe_fn)]
            unsafe fn wake_waker(data: *const ()) {
                drop(Arc::<EnqueueOnClone>::from_raw(data.cast()));
            }
            unsafe fn wake_by_ref_waker(_data: *const ()) {}
            #[allow(unsafe_op_in_unsafe_fn)]
            unsafe fn drop_waker(data: *const ()) {
                drop(Arc::<EnqueueOnClone>::from_raw(data.cast()));
            }
            static VTABLE: RawWakerVTable =
                RawWakerVTable::new(clone_waker, wake_waker, wake_by_ref_waker, drop_waker);

            let writer_waker = unsafe {
                Waker::from_raw(RawWaker::new(
                    Arc::into_raw(Arc::new(EnqueueOnClone(Mutex::new(Some(frame_sender))))).cast(),
                    &VTABLE,
                ))
            };
            let mut write_cx = Context::from_waker(&writer_waker);
            assert!(matches!(
                Pin::new(&mut stream).poll_write(&mut write_cx, b"ping"),
                Poll::Ready(Ok(4))
            ));
            assert!(stream.send_window() > 0);
        });
    }

    #[test]
    fn test_poll_write_registers_same_task_waker_when_reader_is_parked() {
        let rt = rt();
        rt.block_on(async {
            let (mut frame_sender, frame_receiver) = channel(128);
            let (unbound_sender, _unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                1,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );
            stream.send_window = 0;

            let task_fw = Arc::new(FlagWaker::default());
            let task_waker = waker_ref(&task_fw);
            let mut cx = Context::from_waker(&task_waker);
            let mut storage = [0; 1];
            let mut read_buf = ReadBuf::new(&mut storage);
            assert!(
                Pin::new(&mut stream)
                    .poll_read(&mut cx, &mut read_buf)
                    .is_pending()
            );

            assert!(
                Pin::new(&mut stream)
                    .poll_write(&mut cx, b"ping")
                    .is_pending()
            );
            frame_sender
                .try_send(Frame::new_window_update(Flags::default(), 1, 65535))
                .unwrap();
            assert!(
                task_fw.woken(),
                "the blocking drain must register this task's waker"
            );
            assert!(matches!(
                Pin::new(&mut stream).poll_write(&mut cx, b"ping"),
                Poll::Ready(Ok(4))
            ));
            assert!(stream.send_window() > 0);
        });
    }

    // Verifies that when `poll_write` calls `try_recv_frames()` and intercepts an
    // incoming DATA frame (i.e. the read buffer grows), it proactively wakes the
    // parked read task via `readable_wake`.
    //
    // Motivation:
    //   The write path uses `try_recv_frames()` to drain `frame_receiver` non-blockingly
    //   before attempting to write.  When it finds data frames, those frames accumulate
    //   in `read_buf` but the read task is still parked waiting on `frame_receiver`.
    //   Since `try_recv_frames()` does NOT register any waker in `frame_receiver`, the
    //   read task will never receive a wakeup from the channel itself.  Therefore
    //   `recv_frames_wake` must explicitly call `readable_wake.wake()` whenever the
    //   read buffer grows.  Without this, the read side would silently stall even though
    //   data has already arrived and is sitting in `read_buf`.
    //
    // Note: this is a pure positive-behavior test of the `readable_wake` notification
    //   path in `recv_frames_wake`.  It is orthogonal to the waker-overwrite regression
    //   tests above: when a frame is already in the channel, `poll_next` returns `Ready`
    //   immediately without storing a waker, so the overwrite bug does not apply here.
    //   Both the buggy and fixed implementations correctly satisfy this assertion.
    //
    // Test sequence (fully deterministic, no async scheduler involvement):
    //   1. poll_read(read_cx)  → Pending, readable_wake = read_waker.
    //   2. Pre-queue a data frame in frame_sender (already available synchronously).
    //   3. poll_write(write_cx) → try_recv_frames() drains the data frame →
    //      read_buf grows → recv_frames_wake detects buf change →
    //      readable_wake.take().wake() → read_fw.woken() == true.
    #[test]
    fn test_poll_write_wakes_read_when_data_frame_intercepted() {
        let rt = rt();
        rt.block_on(async {
            let (mut frame_sender, frame_receiver) = channel(128);
            let (unbound_sender, _unbound_receiver) = unbounded();
            let mut stream = StreamHandle::new(
                1,
                unbound_sender,
                frame_receiver,
                StreamState::Init,
                INITIAL_STREAM_WINDOW,
            );

            let read_fw = Arc::new(FlagWaker::default());
            let write_fw = Arc::new(FlagWaker::default());
            let read_waker_ref = waker_ref(&read_fw);
            let write_waker_ref = waker_ref(&write_fw);
            let mut read_cx = Context::from_waker(&read_waker_ref);
            let mut write_cx = Context::from_waker(&write_waker_ref);

            // Step 1: poll_read parks → readable_wake = read_waker.
            let mut buf = vec![0u8; 32];
            let mut rbuf = ReadBuf::new(&mut buf);
            assert!(
                Pin::new(&mut stream)
                    .poll_read(&mut read_cx, &mut rbuf)
                    .is_pending(),
                "poll_read must return Pending (no data yet)"
            );
            assert!(!read_fw.woken(), "read_waker must not be woken yet");

            // Step 2: pre-queue a data frame so it is ready for synchronous delivery.
            let frame = Frame::new_data(Flags::from(Flag::Syn), 1, BytesMut::from("hello"));
            frame_sender
                .try_send(frame)
                .expect("channel must accept frame");

            // Step 3: poll_write → recv_frames_wake → try_recv_frames() drains the data
            //   frame synchronously → read_buf grows from 0 to 1 → buf_len check triggers
            //   → readable_wake.take().wake() → read_fw.woken() == true.
            let r = Pin::new(&mut stream).poll_write(&mut write_cx, b"ping");
            assert!(
                matches!(r, Poll::Ready(Ok(4))),
                "poll_write must succeed (send_window has capacity)"
            );

            assert!(
                read_fw.woken(),
                "poll_write must wake the read task after intercepting a data frame via \
                 try_recv_frames(); without this the read side silently stalls even though \
                 data is already sitting in read_buf"
            );

            // Sanity: the data frame really did land in read_buf.
            assert_eq!(
                stream.read_buf.len(),
                1,
                "data frame must be in read_buf after try_recv_frames()"
            );
        });
    }
}
