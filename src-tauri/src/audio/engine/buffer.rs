//! Adaptive latency-target controller (target-occupancy model).
//!
//! The ring buffer is allocated once at the maximum size; this controller only
//! decides how many milliseconds of audio we aim to keep buffered ("target"),
//! growing it when glitches occur (more jitter tolerance, more latency) and
//! shrinking it toward the minimum when the stream has been stable (less
//! latency). It performs no I/O so it is fully unit-testable.

/// Adaptive controller for the buffered-latency target, in milliseconds.
#[derive(Debug, Clone)]
pub struct AdaptiveBuffer {
    min_ms: u32,
    max_ms: u32,
    step_ms: u32,
    target_ms: u32,
    stable_ticks: u32,
    stable_threshold: u32,
}

impl AdaptiveBuffer {
    /// `stable_threshold` = consecutive glitch-free ticks required before the
    /// target is lowered one step. `initial_ms` is clamped into `[min, max]`.
    pub fn new(
        min_ms: u32,
        max_ms: u32,
        step_ms: u32,
        initial_ms: u32,
        stable_threshold: u32,
    ) -> Self {
        let max_ms = max_ms.max(min_ms);
        Self {
            min_ms,
            max_ms,
            step_ms: step_ms.max(1),
            target_ms: initial_ms.clamp(min_ms, max_ms),
            stable_ticks: 0,
            stable_threshold: stable_threshold.max(1),
        }
    }

    pub fn target_ms(&self) -> u32 {
        self.target_ms
    }

    /// Advances one control tick. `had_glitch` = an underrun or overrun was
    /// observed since the previous tick. Returns `Some(new_target)` only when
    /// the target actually changed.
    pub fn on_tick(&mut self, had_glitch: bool) -> Option<u32> {
        let prev = self.target_ms;
        if had_glitch {
            self.stable_ticks = 0;
            self.target_ms = (self.target_ms + self.step_ms).min(self.max_ms);
        } else {
            self.stable_ticks += 1;
            if self.stable_ticks >= self.stable_threshold {
                self.stable_ticks = 0;
                self.target_ms = self.target_ms.saturating_sub(self.step_ms).max(self.min_ms);
            }
        }
        (self.target_ms != prev).then_some(self.target_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_target_is_clamped() {
        assert_eq!(
            AdaptiveBuffer::new(2000, 8000, 500, 500, 3).target_ms(),
            2000
        );
        assert_eq!(
            AdaptiveBuffer::new(2000, 8000, 500, 99000, 3).target_ms(),
            8000
        );
        assert_eq!(
            AdaptiveBuffer::new(2000, 8000, 500, 4000, 3).target_ms(),
            4000
        );
    }

    #[test]
    fn glitch_raises_target_up_to_max() {
        let mut a = AdaptiveBuffer::new(2000, 3000, 500, 2000, 3);
        assert_eq!(a.on_tick(true), Some(2500));
        assert_eq!(a.on_tick(true), Some(3000));
        assert_eq!(a.on_tick(true), None); // already at max, no change
        assert_eq!(a.target_ms(), 3000);
    }

    #[test]
    fn stability_lowers_target_after_threshold() {
        let mut a = AdaptiveBuffer::new(2000, 8000, 500, 3000, 3);
        assert_eq!(a.on_tick(false), None); // 1
        assert_eq!(a.on_tick(false), None); // 2
        assert_eq!(a.on_tick(false), Some(2500)); // 3rd glitch-free tick lowers
        assert_eq!(a.on_tick(false), None); // counter reset
        assert_eq!(a.on_tick(false), None);
        assert_eq!(a.on_tick(false), Some(2000)); // lowers again
        assert_eq!(a.on_tick(false), None); // already at min after threshold
        assert_eq!(a.on_tick(false), None);
        assert_eq!(a.on_tick(false), None); // floored at min, no change
    }

    #[test]
    fn glitch_resets_stability_counter() {
        let mut a = AdaptiveBuffer::new(2000, 8000, 500, 3000, 3);
        a.on_tick(false);
        a.on_tick(false);
        a.on_tick(true); // raises to 3500, resets counter
        assert_eq!(a.target_ms(), 3500);
        assert_eq!(a.on_tick(false), None); // counter restarts from 0
        assert_eq!(a.on_tick(false), None);
        assert_eq!(a.on_tick(false), Some(3000));
    }
}
