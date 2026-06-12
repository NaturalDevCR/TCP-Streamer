//! Pure pacing helpers for the engine send loop: standing-latency enforcement
//! (how much backlog to drop) and honest underrun detection (sustained
//! starvation, not instantaneous emptiness).

/// Samples to drop so ring occupancy returns to the standing-latency target.
///
/// Returns 0 while `occupied` is within `target_ms + margin_ms`; above that
/// threshold it returns the samples in excess of `target_ms` (we drain back
/// to the target, not to the threshold, so small jitter doesn't trigger a
/// drop on every iteration), rounded DOWN to a whole frame so skipping never
/// breaks channel interleaving (an odd skip would swap L/R for the rest of
/// the stream).
pub fn excess_samples(
    occupied: usize,
    target_ms: u32,
    margin_ms: u32,
    sample_rate: u32,
    channels: u16,
) -> usize {
    let per_sec = sample_rate as usize * channels as usize;
    let target = per_sec * target_ms as usize / 1000;
    let threshold = per_sec * (target_ms + margin_ms) as usize / 1000;
    if occupied > threshold {
        let raw = occupied - target;
        raw - (raw % channels.max(1) as usize)
    } else {
        0
    }
}

/// One-shot detector for sustained buffer starvation.
///
/// The send loop runs far faster than capture fills the ring, so an empty ring
/// at any instant is normal. A real underrun is emptiness that PERSISTS while
/// we are connected. This gate fires once per episode, after `threshold_ms` of
/// continuous starvation, and re-arms only after the buffer recovers.
pub struct StarvationGate {
    threshold_ms: u64,
    empty_since_ms: Option<u64>,
    fired: bool,
}

impl StarvationGate {
    pub fn new(threshold_ms: u64) -> Self {
        Self {
            threshold_ms,
            empty_since_ms: None,
            fired: false,
        }
    }

    /// Advances the detector. `now_ms` is any monotonic millisecond clock.
    /// Returns true exactly once per sustained-starvation episode.
    pub fn update(&mut self, is_starved: bool, now_ms: u64) -> bool {
        if !is_starved {
            self.empty_since_ms = None;
            self.fired = false;
            return false;
        }
        let since = *self.empty_since_ms.get_or_insert(now_ms);
        if !self.fired && now_ms.saturating_sub(since) >= self.threshold_ms {
            self.fired = true;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48000;

    #[test]
    fn no_excess_within_target_plus_margin() {
        // target 300ms stereo @48k = 28800 samples; margin 250ms = 24000.
        let at_target = 48000 * 2 * 300 / 1000;
        let just_below_threshold = 48000 * 2 * 540 / 1000;
        assert_eq!(excess_samples(at_target, 300, 250, RATE, 2), 0);
        assert_eq!(excess_samples(just_below_threshold, 300, 250, RATE, 2), 0);
        assert_eq!(excess_samples(0, 300, 250, RATE, 2), 0);
    }

    #[test]
    fn excess_above_threshold_drains_back_to_target() {
        // 2 seconds buffered, target 300ms (+250ms margin) → drop down to 300ms.
        let occupied = 48000 * 2 * 2000 / 1000;
        let target_samples = 48000 * 2 * 300 / 1000;
        assert_eq!(
            excess_samples(occupied, 300, 250, RATE, 2),
            occupied - target_samples
        );
    }

    #[test]
    fn excess_is_always_frame_aligned() {
        // An occupancy/target combination that would yield an odd raw excess
        // must round down to a whole stereo frame, or cons.skip() would swap
        // L/R permanently.
        let occupied = 48000 * 2; // 1s stereo
        for target_ms in [3, 5, 7, 333] {
            let excess = excess_samples(occupied, target_ms, 0, 44100, 2);
            assert_eq!(excess % 2, 0, "target_ms={target_ms} gave odd excess");
            assert!(excess > 0);
        }
        // 6-channel ring: excess must be a multiple of 6.
        let excess = excess_samples(100_003, 100, 0, 48000, 6);
        assert_eq!(excess % 6, 0);
    }

    #[test]
    fn excess_scales_with_channels_and_rate() {
        // mono 44.1k: 1 second buffered, target 200ms, margin 100ms.
        let occupied = 44100;
        let target_samples = 44100 * 200 / 1000;
        assert_eq!(
            excess_samples(occupied, 200, 100, 44100, 1),
            occupied - target_samples
        );
    }

    #[test]
    fn gate_does_not_fire_before_threshold() {
        let mut g = StarvationGate::new(250);
        assert!(!g.update(true, 0));
        assert!(!g.update(true, 100));
        assert!(!g.update(true, 249));
    }

    #[test]
    fn gate_fires_once_at_threshold() {
        let mut g = StarvationGate::new(250);
        assert!(!g.update(true, 0));
        assert!(g.update(true, 250));
        // Still starved: must not fire again within the same episode.
        assert!(!g.update(true, 300));
        assert!(!g.update(true, 10_000));
    }

    #[test]
    fn gate_rearms_after_recovery() {
        let mut g = StarvationGate::new(250);
        assert!(!g.update(true, 0));
        assert!(g.update(true, 250));
        // Recovery resets the episode.
        assert!(!g.update(false, 400));
        // New episode starts counting from its own beginning.
        assert!(!g.update(true, 500));
        assert!(!g.update(true, 700));
        assert!(g.update(true, 750));
    }

    #[test]
    fn gate_recovery_before_threshold_never_fires() {
        let mut g = StarvationGate::new(250);
        assert!(!g.update(true, 0));
        assert!(!g.update(false, 100));
        assert!(!g.update(true, 200));
        assert!(!g.update(true, 449)); // episode is only 249ms old
        assert!(g.update(true, 450));
    }
}
