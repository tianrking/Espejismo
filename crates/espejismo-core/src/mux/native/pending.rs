use std::collections::VecDeque;

use anyhow::{bail, Result};

use crate::protocol::request::StreamPriority;

// Keep interactive traffic responsive while bounding bulk starvation when both
// classes stay backlogged. Control frames remain ahead of either data class.
const MAX_INTERACTIVE_BURST: usize = 8;

pub(super) struct PendingFrame {
    pub(super) kind: u8,
    pub(super) stream_id: u32,
    pub(super) payload: Vec<u8>,
    pub(super) queued_stream: Option<u32>,
}

pub(super) struct PendingFrames {
    control: VecDeque<PendingFrame>,
    interactive: VecDeque<PendingFrame>,
    bulk: VecDeque<PendingFrame>,
    len: usize,
    limit: usize,
    interactive_burst: usize,
}

impl PendingFrames {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            control: VecDeque::new(),
            interactive: VecDeque::new(),
            bulk: VecDeque::new(),
            len: 0,
            limit: limit.max(1),
            interactive_burst: 0,
        }
    }

    pub(super) fn push_control(&mut self, frame: PendingFrame) -> Result<()> {
        self.reserve_slot()?;
        self.control.push_back(frame);
        Ok(())
    }

    pub(super) fn push_data(
        &mut self,
        priority: StreamPriority,
        frame: PendingFrame,
    ) -> Result<()> {
        self.reserve_slot()?;
        match priority {
            StreamPriority::Interactive => self.interactive.push_back(frame),
            StreamPriority::Bulk => self.bulk.push_back(frame),
        }
        Ok(())
    }

    pub(super) fn pop_next(&mut self) -> Option<PendingFrame> {
        let frame = if let Some(frame) = self.control.pop_front() {
            Some(frame)
        } else if !self.bulk.is_empty()
            && (self.interactive.is_empty() || self.interactive_burst >= MAX_INTERACTIVE_BURST)
        {
            self.interactive_burst = 0;
            self.bulk.pop_front()
        } else if let Some(frame) = self.interactive.pop_front() {
            if !self.bulk.is_empty() {
                self.interactive_burst += 1;
            }
            Some(frame)
        } else {
            self.interactive_burst = 0;
            self.bulk.pop_front()
        };
        if frame.is_some() {
            self.len = self.len.saturating_sub(1);
        }
        frame
    }

    /// Queue occupancy ratio as a percentage, rounded up so any non-empty
    /// queue with a tiny limit reports a visible watermark.
    #[cfg(test)]
    pub(super) fn occupancy_percent(&self) -> usize {
        self.len.saturating_mul(100).div_ceil(self.limit)
    }

    /// Signal sustained queue pressure at 75% occupancy. This is advisory;
    /// admission remains bounded by the hard frame limit below.
    pub(super) fn is_congested(&self) -> bool {
        self.len.saturating_mul(4) >= self.limit.saturating_mul(3)
    }

    fn reserve_slot(&mut self) -> Result<()> {
        if self.len >= self.limit {
            if self.is_congested() {
                bail!("native mux pending frame queue congested: limit reached");
            }
            bail!("native mux pending frame queue limit reached");
        }
        self.len += 1;
        Ok(())
    }
}
