use std::cmp;
use std::time::Duration;

use anyhow::{bail, Result};
use rand::Rng;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::{sleep, Instant};

use crate::crypto::{decrypt, encrypt, SessionKeys};

const MAX_FRAME: usize = 64 * 1024;
const DATA_CHUNK: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FrameType {
    Target = 1,
    Data = 2,
    Close = 3,
    Padding = 4,
}

impl TryFrom<u8> for FrameType {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Target),
            2 => Ok(Self::Data),
            3 => Ok(Self::Close),
            4 => Ok(Self::Padding),
            _ => bail!("unknown frame type {value}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub ty: FrameType,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct FrameOptions {
    pub max_padding: usize,
    pub jitter_ms: u64,
    pub padding_chance_percent: u8,
    pub backpressure_threshold_ms: u64,
    pub backpressure_cooldown_ms: u64,
}

impl Default for FrameOptions {
    fn default() -> Self {
        Self {
            max_padding: 64,
            jitter_ms: 0,
            padding_chance_percent: 35,
            backpressure_threshold_ms: 40,
            backpressure_cooldown_ms: 1000,
        }
    }
}

pub struct FrameCodec<S> {
    stream: S,
    tx_seq: u64,
    rx_seq: u64,
    keys: SessionKeys,
    options: FrameOptions,
    padding_disabled_until: Option<Instant>,
}

pub struct FrameWriter<W> {
    writer: W,
    tx_seq: u64,
    keys: SessionKeys,
    options: FrameOptions,
    padding_disabled_until: Option<Instant>,
}

pub struct FrameReader<R> {
    reader: R,
    rx_seq: u64,
    keys: SessionKeys,
}

impl<S> FrameCodec<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(stream: S, keys: SessionKeys, options: FrameOptions) -> Self {
        Self {
            stream,
            tx_seq: 0,
            rx_seq: 0,
            keys,
            options,
            padding_disabled_until: None,
        }
    }

    pub async fn send(&mut self, frame: Frame) -> Result<()> {
        if frame.ty != FrameType::Padding {
            self.maybe_send_padding().await?;
        }
        let elapsed = write_one(
            &mut self.stream,
            &self.keys,
            &mut self.tx_seq,
            &self.options,
            frame,
        )
        .await?;
        self.observe_backpressure(elapsed);
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<Frame> {
        loop {
            let frame = read_one(&mut self.stream, &self.keys, &mut self.rx_seq).await?;
            if frame.ty != FrameType::Padding {
                return Ok(frame);
            }
        }
    }

    pub fn into_inner(self) -> S {
        self.stream
    }

    async fn maybe_send_padding(&mut self) -> Result<()> {
        if !should_send_padding(&self.options, self.padding_disabled_until) {
            return Ok(());
        }
        let len = rand::thread_rng().gen_range(1..=self.options.max_padding);
        let payload = patterned_padding(len);
        let elapsed = write_one(
            &mut self.stream,
            &self.keys,
            &mut self.tx_seq,
            &self.options,
            Frame {
                ty: FrameType::Padding,
                payload,
            },
        )
        .await?;
        self.observe_backpressure(elapsed);
        Ok(())
    }

    fn observe_backpressure(&mut self, elapsed: Duration) {
        if let Some(until) = observe_backpressure(&self.options, elapsed) {
            self.padding_disabled_until = Some(until);
        }
    }
}

impl<W> FrameWriter<W>
where
    W: AsyncWrite + Unpin,
{
    pub fn new(writer: W, keys: SessionKeys, options: FrameOptions) -> Self {
        Self {
            writer,
            tx_seq: 0,
            keys,
            options,
            padding_disabled_until: None,
        }
    }

    pub async fn send(&mut self, frame: Frame) -> Result<()> {
        if frame.ty != FrameType::Padding {
            self.maybe_send_padding().await?;
        }
        let elapsed = write_one(
            &mut self.writer,
            &self.keys,
            &mut self.tx_seq,
            &self.options,
            frame,
        )
        .await?;
        self.observe_backpressure(elapsed);
        Ok(())
    }

    async fn maybe_send_padding(&mut self) -> Result<()> {
        if !should_send_padding(&self.options, self.padding_disabled_until) {
            return Ok(());
        }
        let len = rand::thread_rng().gen_range(1..=self.options.max_padding);
        let elapsed = write_one(
            &mut self.writer,
            &self.keys,
            &mut self.tx_seq,
            &self.options,
            Frame {
                ty: FrameType::Padding,
                payload: patterned_padding(len),
            },
        )
        .await?;
        self.observe_backpressure(elapsed);
        Ok(())
    }

    fn observe_backpressure(&mut self, elapsed: Duration) {
        if let Some(until) = observe_backpressure(&self.options, elapsed) {
            self.padding_disabled_until = Some(until);
        }
    }
}

impl<R> FrameReader<R>
where
    R: AsyncRead + Unpin,
{
    pub fn new(reader: R, keys: SessionKeys) -> Self {
        Self {
            reader,
            rx_seq: 0,
            keys,
        }
    }

    pub async fn recv(&mut self) -> Result<Frame> {
        loop {
            let frame = read_one(&mut self.reader, &self.keys, &mut self.rx_seq).await?;
            if frame.ty != FrameType::Padding {
                return Ok(frame);
            }
        }
    }
}

pub async fn send_frame<S>(codec: &mut FrameCodec<S>, ty: FrameType, payload: Vec<u8>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    codec.send(Frame { ty, payload }).await
}

pub async fn read_frame<S>(codec: &mut FrameCodec<S>) -> Result<Frame>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    codec.recv().await
}

pub async fn copy_encrypted<R, S>(mut reader: R, codec: &mut FrameCodec<S>) -> Result<u64>
where
    R: AsyncRead + Unpin,
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut buf = vec![0_u8; DATA_CHUNK];
    let mut total = 0_u64;
    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            codec
                .send(Frame {
                    ty: FrameType::Close,
                    payload: Vec::new(),
                })
                .await?;
            return Ok(total);
        }
        total += n as u64;
        codec
            .send(Frame {
                ty: FrameType::Data,
                payload: buf[..n].to_vec(),
            })
            .await?;
    }
}

async fn write_one<S>(
    stream: &mut S,
    keys: &SessionKeys,
    tx_seq: &mut u64,
    options: &FrameOptions,
    frame: Frame,
) -> Result<Duration>
where
    S: AsyncWrite + Unpin,
{
    maybe_jitter(options).await;
    let started = Instant::now();
    let mut plain = Vec::with_capacity(frame.payload.len() + 1);
    plain.push(frame.ty as u8);
    plain.extend_from_slice(&frame.payload);
    let encrypted = encrypt(&keys.tx, *tx_seq, &plain)?;
    *tx_seq += 1;
    if encrypted.len() > MAX_FRAME {
        bail!("encrypted frame too large");
    }
    stream
        .write_all(&(encrypted.len() as u32).to_be_bytes())
        .await?;
    stream.write_all(&encrypted).await?;
    Ok(started.elapsed())
}

async fn read_one<S>(stream: &mut S, keys: &SessionKeys, rx_seq: &mut u64) -> Result<Frame>
where
    S: AsyncRead + Unpin,
{
    let len = stream.read_u32().await? as usize;
    if len == 0 || len > MAX_FRAME {
        bail!("invalid frame length {len}");
    }
    let mut encrypted = vec![0_u8; len];
    stream.read_exact(&mut encrypted).await?;
    let plain = decrypt(&keys.rx, *rx_seq, &encrypted)?;
    *rx_seq += 1;
    if plain.is_empty() {
        bail!("empty plaintext frame");
    }
    Ok(Frame {
        ty: FrameType::try_from(plain[0])?,
        payload: plain[1..].to_vec(),
    })
}

async fn maybe_jitter(options: &FrameOptions) {
    if options.jitter_ms == 0 {
        return;
    }
    let upper = cmp::max(1, options.jitter_ms);
    let delay = rand::thread_rng().gen_range(0..=upper);
    if delay > 0 {
        sleep(Duration::from_millis(delay)).await;
    }
}

fn patterned_padding(len: usize) -> Vec<u8> {
    let mut payload = vec![0_u8; len];
    for (idx, byte) in payload.iter_mut().enumerate() {
        *byte = if idx % 9 == 0 { b':' } else { b'0' };
    }
    payload
}

fn should_send_padding(options: &FrameOptions, disabled_until: Option<Instant>) -> bool {
    if options.max_padding == 0 || options.padding_chance_percent == 0 {
        return false;
    }
    if disabled_until.is_some_and(|until| Instant::now() < until) {
        return false;
    }
    let chance = f64::from(options.padding_chance_percent.min(100)) / 100.0;
    rand::thread_rng().gen_bool(chance)
}

fn observe_backpressure(options: &FrameOptions, elapsed: Duration) -> Option<Instant> {
    if options.backpressure_threshold_ms == 0 || options.backpressure_cooldown_ms == 0 {
        return None;
    }
    if elapsed >= Duration::from_millis(options.backpressure_threshold_ms) {
        return Some(Instant::now() + Duration::from_millis(options.backpressure_cooldown_ms));
    }
    None
}
