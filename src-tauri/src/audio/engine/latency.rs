//! Latency profile → buffer parameters (pure, testable).
//!
//! Model: `ring_ms` is the ring CAPACITY — how much audio we can hold without
//! dropping capture data during a network stall. The adaptive band
//! (`adaptive_min_ms`..`adaptive_max_ms`) is the STANDING-LATENCY target: the
//! engine drops backlog above the current target to keep end-to-end latency
//! bounded, and the AdaptiveBuffer controller moves the target inside this band
//! based on observed glitches. `prefill_ms` is a small startup cushion before
//! the first byte is sent — it must stay small or it becomes permanent latency.
//! Loopback (WASAPI) capture gets higher floors because it needs more buffering.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencyParams {
    pub ring_ms: u32,
    pub adaptive_min_ms: u32,
    pub adaptive_max_ms: u32,
    pub chunk_size: u32,
    pub prefill_ms: u32,
}

/// Returns buffer parameters for a named profile. Unknown profiles fall back to
/// "balanced". Use this only for the named profiles; "custom" is handled by the
/// caller, which passes the user's manual fields instead.
pub fn params(profile: &str, is_loopback: bool) -> LatencyParams {
    let (ring, amin, amax, chunk, prefill) = match (profile, is_loopback) {
        ("ultra-low", false) => (2000, 100, 500, 256, 100),
        ("ultra-low", true) => (3000, 150, 800, 256, 150),
        ("robust", false) => (8000, 500, 3000, 1024, 300),
        ("robust", true) => (10000, 800, 4000, 1024, 400),
        // "balanced" and any unknown profile
        (_, false) => (4000, 200, 1500, 512, 200),
        (_, true) => (6000, 300, 2000, 512, 250),
    };
    LatencyParams {
        ring_ms: ring,
        adaptive_min_ms: amin,
        adaptive_max_ms: amax,
        chunk_size: chunk,
        prefill_ms: prefill,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ultra_low_has_lowest_latency() {
        assert!(params("ultra-low", false).ring_ms < params("balanced", false).ring_ms);
        assert!(params("ultra-low", false).chunk_size <= params("balanced", false).chunk_size);
    }

    #[test]
    fn robust_buffers_more_than_balanced() {
        assert!(params("robust", false).ring_ms > params("balanced", false).ring_ms);
        assert!(
            params("robust", false).adaptive_min_ms > params("balanced", false).adaptive_min_ms
        );
    }

    #[test]
    fn unknown_profile_falls_back_to_balanced() {
        assert_eq!(params("nonsense", false), params("balanced", false));
    }

    #[test]
    fn loopback_floors_are_higher_than_non_loopback() {
        for p in ["ultra-low", "balanced", "robust"] {
            assert!(
                params(p, true).ring_ms >= params(p, false).ring_ms,
                "loopback ring for {p} should be >= non-loopback"
            );
            assert!(params(p, true).adaptive_min_ms >= params(p, false).adaptive_min_ms);
        }
    }

    #[test]
    fn prefill_is_small_for_instant_start() {
        for p in ["ultra-low", "balanced", "robust"] {
            for lb in [false, true] {
                let lp = params(p, lb);
                assert!(
                    lp.prefill_ms <= 400,
                    "prefill for {p} (loopback={lb}) must be <=400ms for fast startup, got {}",
                    lp.prefill_ms
                );
                assert!(
                    lp.prefill_ms < lp.ring_ms,
                    "prefill must be far below ring capacity for {p}"
                );
            }
        }
    }

    #[test]
    fn target_ceiling_fits_inside_ring() {
        for p in ["ultra-low", "balanced", "robust"] {
            for lb in [false, true] {
                let lp = params(p, lb);
                assert!(
                    lp.adaptive_max_ms <= lp.ring_ms,
                    "adaptive target ceiling must fit in ring capacity for {p} (loopback={lb}): {} > {}",
                    lp.adaptive_max_ms,
                    lp.ring_ms
                );
            }
        }
    }

    #[test]
    fn target_floor_keeps_latency_low() {
        for p in ["ultra-low", "balanced", "robust"] {
            let lp = params(p, false);
            assert!(
                lp.adaptive_min_ms <= 1000,
                "standing-latency floor for {p} must be <=1000ms, got {}",
                lp.adaptive_min_ms
            );
        }
    }
}
