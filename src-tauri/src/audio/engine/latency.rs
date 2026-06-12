//! Latency profile → buffer parameters (pure, testable).
//!
//! Replaces the previous hardcoded buffer floors. Each named profile trades
//! latency against jitter tolerance; loopback (WASAPI) capture gets higher
//! floors because it needs more buffering. `prefill_ms` equals `ring_ms`.

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
    let (ring, amin, amax, chunk) = match (profile, is_loopback) {
        ("ultra-low", false) => (500, 300, 2000, 256),
        ("ultra-low", true) => (1500, 1000, 4000, 256),
        ("robust", false) => (8000, 5000, 15000, 1024),
        ("robust", true) => (12000, 8000, 20000, 1024),
        // "balanced" and any unknown profile
        (_, false) => (4000, 2000, 10000, 512),
        (_, true) => (8000, 4000, 10000, 512),
    };
    LatencyParams {
        ring_ms: ring,
        adaptive_min_ms: amin,
        adaptive_max_ms: amax,
        chunk_size: chunk,
        prefill_ms: ring,
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
    fn prefill_equals_ring() {
        let lp = params("balanced", false);
        assert_eq!(lp.prefill_ms, lp.ring_ms);
    }
}
