//! Clock-drift controller (no PTP). Tracks an EMA of the playback buffer level;
//! when it trends above target it asks to drop a mini-chunk (sink clock slower
//! than source), below target it asks to insert silence (sink faster). A
//! cooldown keeps corrections rare so they are inaudible micro-glitches.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftAction {
    None,
    DropChunk,
    InsertSilence,
}

pub struct DriftController {
    target: f32,
    margin: f32,
    alpha: f32,
    ema: f32,
    cooldown: u32,
    since: u32,
    primed: bool,
}

impl DriftController {
    /// `target`/`margin` are in the same unit you pass to `observe` (e.g. buffered
    /// frames). `cooldown` ticks must pass between corrections.
    pub fn new(target: f32, margin: f32, cooldown: u32) -> Self {
        Self { target, margin, alpha: 0.05, ema: target, cooldown, since: 0, primed: false, }
    }

    /// Feed the current buffered level; returns the correction to apply.
    pub fn observe(&mut self, buffered: f32) -> DriftAction {
        self.ema = if self.primed {
            self.alpha * buffered + (1.0 - self.alpha) * self.ema
        } else {
            self.primed = true;
            buffered
        };
        if self.since < self.cooldown {
            self.since += 1;
            return DriftAction::None;
        }
        if self.ema > self.target + self.margin {
            self.since = 0;
            DriftAction::DropChunk
        } else if self.ema < self.target - self.margin {
            self.since = 0;
            DriftAction::InsertSilence
        } else {
            DriftAction::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive(c: &mut DriftController, level: f32, n: usize) -> DriftAction {
        let mut last = DriftAction::None;
        for _ in 0..n {
            last = c.observe(level);
        }
        last
    }

    #[test]
    fn high_buffer_asks_to_drop() {
        let mut c = DriftController::new(100.0, 20.0, 2);
        assert_eq!(drive(&mut c, 200.0, 3), DriftAction::DropChunk);
    }

    #[test]
    fn low_buffer_asks_to_insert() {
        let mut c = DriftController::new(100.0, 20.0, 2);
        assert_eq!(drive(&mut c, 10.0, 3), DriftAction::InsertSilence);
    }

    #[test]
    fn on_target_does_nothing() {
        let mut c = DriftController::new(100.0, 20.0, 2);
        assert_eq!(drive(&mut c, 100.0, 3), DriftAction::None);
    }

    #[test]
    fn cooldown_prevents_back_to_back_corrections() {
        let mut c = DriftController::new(100.0, 20.0, 5);
        // First correction fires once ema crosses; immediately after, cooldown holds.
        let _ = drive(&mut c, 200.0, 6);
        assert_eq!(c.observe(200.0), DriftAction::None); // within cooldown
    }
}
