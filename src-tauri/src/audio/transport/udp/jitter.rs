//! De-jitter / reorder buffer keyed by sequence number (pure, testable).
//!
//! The source numbers audio frames 0,1,2,... The sink inserts them (possibly
//! out of order) and pulls them in order for playback. A frame still missing
//! once `lost_after` later frames have arrived is declared lost (a gap → the
//! caller plays silence). Frames older than the next expected one are dropped.

use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
pub enum Pop {
    /// The next in-order frame's payload.
    Frame(Vec<u8>),
    /// The next frame is declared lost; play silence and advance.
    Gap,
    /// Not enough buffered yet; play silence but keep waiting.
    Starved,
}

pub struct JitterBuffer {
    next_seq: u64,
    started: bool,
    buf: BTreeMap<u64, Vec<u8>>,
    lost_after: usize,
}

impl JitterBuffer {
    /// `lost_after` = how many later frames must be buffered before a missing
    /// `next_seq` is treated as lost. The stream synchronizes to its first received sequence; discontinuities are bounded.
    pub fn new(lost_after: usize) -> Self {
        Self {
            next_seq: 0,
            started: false,
            buf: BTreeMap::new(),
            lost_after: lost_after.max(1),
        }
    }

    pub fn push(&mut self, seq: u64, frame: Vec<u8>) {
        if !self.started {
            self.next_seq = seq;
            self.started = true;
        }
        // Rebase large discontinuities in constant work and bound retained memory.
        if seq.saturating_sub(self.next_seq) > 256 {
            self.buf.clear();
            self.next_seq = seq;
        }
        if self.buf.len() >= 256 && !self.buf.contains_key(&seq) {
            return;
        }
        if seq < self.next_seq {
            return; // too old
        }
        self.buf.insert(seq, frame);
    }

    pub fn pop(&mut self) -> Pop {
        if let Some(frame) = self.buf.remove(&self.next_seq) {
            self.next_seq = self.next_seq.saturating_add(1);
            return Pop::Frame(frame);
        }
        if self.buf.len() >= self.lost_after {
            self.next_seq = self.next_seq.saturating_add(1); // declare lost, skip the gap
            return Pop::Gap;
        }
        Pop::Starved
    }

    #[allow(dead_code)]
    pub fn buffered(&self) -> usize {
        self.buf.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(b: u8) -> Vec<u8> {
        vec![b]
    }

    #[test]
    fn in_order() {
        let mut j = JitterBuffer::new(3);
        j.push(0, f(0));
        j.push(1, f(1));
        assert_eq!(j.pop(), Pop::Frame(f(0)));
        assert_eq!(j.pop(), Pop::Frame(f(1)));
        assert_eq!(j.pop(), Pop::Starved);
    }

    #[test]
    fn out_of_order_is_reordered() {
        let mut j = JitterBuffer::new(3);
        j.push(0, f(0));
        j.push(2, f(2));
        j.push(1, f(1));
        assert_eq!(j.pop(), Pop::Frame(f(0)));
        assert_eq!(j.pop(), Pop::Frame(f(1)));
        assert_eq!(j.pop(), Pop::Frame(f(2)));
    }

    #[test]
    fn missing_frame_becomes_gap_once_enough_buffered() {
        let mut j = JitterBuffer::new(2);
        j.push(0, f(0));
        assert_eq!(j.pop(), Pop::Frame(f(0)));
        // seq 1 never arrives; 2 and 3 do.
        j.push(2, f(2));
        j.push(3, f(3));
        assert_eq!(j.pop(), Pop::Gap);
        assert_eq!(j.pop(), Pop::Frame(f(2)));
        assert_eq!(j.pop(), Pop::Frame(f(3)));
    }

    #[test]
    fn waits_when_not_enough_buffered() {
        let mut j = JitterBuffer::new(3);
        j.push(0, f(0));
        assert_eq!(j.pop(), Pop::Frame(f(0)));
        j.push(2, f(2));
        assert_eq!(j.pop(), Pop::Starved);
        j.push(1, f(1));
        assert_eq!(j.pop(), Pop::Frame(f(1)));
        assert_eq!(j.pop(), Pop::Frame(f(2)));
    }

    #[test]
    fn drops_stale_frames() {
        let mut j = JitterBuffer::new(2);
        j.push(0, f(0));
        assert_eq!(j.pop(), Pop::Frame(f(0))); // next_seq now 1
        j.push(0, f(9)); // stale
        assert_eq!(j.buffered(), 0);
    }
}
