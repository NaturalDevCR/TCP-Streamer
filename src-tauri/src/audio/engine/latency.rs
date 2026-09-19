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
        ("broadcast", false) => (3000, 250, 250, 512, 250),
        ("broadcast", true) => (4000, 400, 400, 512, 400),
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

/// Bounds for the broadcast profile's fixed-latency override, in milliseconds.
/// Values outside the range are clamped, never rejected: a bad number should
/// degrade the stream, not refuse to start it.
// Not yet called from production code: the source engine and sink are wired
// to `resolve()` in a later task. `#[allow(dead_code)]` matches the existing
// convention for pending-integration code in this crate (see
// `engine::capture::resolve_buffer_size`).
#[allow(dead_code)]
pub const FIXED_LATENCY_MIN_MS: u32 = 50;
#[allow(dead_code)]
pub const FIXED_LATENCY_MAX_MS: u32 = 2000;

/// Manual latency inputs supplied by the user. `None` means "use the named
/// profile's built-in value", so a caller that knows nothing about a field can
/// omit it.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LatencyOverrides {
    pub ring_ms: Option<u32>,
    pub min_buffer_ms: Option<u32>,
    pub max_buffer_ms: Option<u32>,
    pub chunk_size: Option<u32>,
    pub fixed_latency_ms: Option<u32>,
}

/// Resolves a profile name plus user overrides into concrete buffer parameters.
///
/// This is the ONLY place profile resolution happens. The source engine and the
/// sink both call it, so they cannot drift apart — they did before, which is how
/// "custom" came to be silently ignored on the sink side.
///
/// Overrides apply only to the profiles that define them: `custom` reads the
/// manual buffer fields, `broadcast` reads `fixed_latency_ms`, and every named
/// profile ignores all of them.
#[allow(dead_code)]
pub fn resolve(profile: &str, is_loopback: bool, overrides: LatencyOverrides) -> LatencyParams {
    match profile {
        "custom" => {
            // Defaults mirror the frontend store's initial values so an omitted
            // field behaves the same as an untouched slider.
            let ring_ms = overrides.ring_ms.unwrap_or(4000);
            LatencyParams {
                ring_ms,
                adaptive_min_ms: overrides.min_buffer_ms.unwrap_or(2000),
                adaptive_max_ms: overrides.max_buffer_ms.unwrap_or(10000),
                chunk_size: overrides.chunk_size.unwrap_or(512),
                prefill_ms: ring_ms.min(200),
            }
        }
        "broadcast" => {
            let base = params("broadcast", is_loopback);
            match overrides.fixed_latency_ms {
                Some(f) => {
                    let fixed = f.clamp(FIXED_LATENCY_MIN_MS, FIXED_LATENCY_MAX_MS);
                    LatencyParams {
                        // Ring is stall-absorption capacity, not latency. Keep the
                        // profile's floor, and grow it for large fixed targets.
                        ring_ms: base.ring_ms.max(fixed.saturating_mul(4)),
                        adaptive_min_ms: fixed,
                        adaptive_max_ms: fixed,
                        chunk_size: base.chunk_size,
                        // Start already at the standing latency instead of climbing
                        // to it, so the operator's first A/V measurement is correct.
                        prefill_ms: fixed.min(400),
                    }
                }
                None => base,
            }
        }
        _ => params(profile, is_loopback),
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
        for p in ["ultra-low", "balanced", "robust", "broadcast"] {
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
        for p in ["ultra-low", "balanced", "robust", "broadcast"] {
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

    #[test]
    fn broadcast_band_is_collapsed() {
        for lb in [false, true] {
            let lp = params("broadcast", lb);
            assert_eq!(
                lp.adaptive_min_ms, lp.adaptive_max_ms,
                "broadcast must have a collapsed adaptive band (loopback={lb})"
            );
        }
        assert_eq!(params("broadcast", false).adaptive_min_ms, 250);
        assert_eq!(params("broadcast", true).adaptive_min_ms, 400);
    }

    #[test]
    fn resolve_matches_params_for_named_profiles() {
        // Snapcast regression guard: resolve() must be a no-op wrapper for every
        // profile that existed before the broadcast work.
        for p in ["ultra-low", "balanced", "robust", "nonsense"] {
            for lb in [false, true] {
                assert_eq!(
                    resolve(p, lb, LatencyOverrides::default()),
                    params(p, lb),
                    "resolve must equal params for {p} (loopback={lb})"
                );
            }
        }
    }

    #[test]
    fn resolve_custom_matches_previous_engine_behavior() {
        // Reproduces the struct the source engine built inline before the refactor.
        let got = resolve(
            "custom",
            false,
            LatencyOverrides {
                ring_ms: Some(6000),
                min_buffer_ms: Some(300),
                max_buffer_ms: Some(2500),
                chunk_size: Some(1024),
                fixed_latency_ms: None,
            },
        );
        assert_eq!(
            got,
            LatencyParams {
                ring_ms: 6000,
                adaptive_min_ms: 300,
                adaptive_max_ms: 2500,
                chunk_size: 1024,
                prefill_ms: 200,
            }
        );
    }

    #[test]
    fn resolve_custom_prefill_follows_small_ring() {
        let got = resolve(
            "custom",
            false,
            LatencyOverrides {
                ring_ms: Some(150),
                ..LatencyOverrides::default()
            },
        );
        assert_eq!(got.prefill_ms, 150, "prefill is ring_ms.min(200)");
    }

    #[test]
    fn broadcast_override_sets_fixed_target() {
        let got = resolve(
            "broadcast",
            false,
            LatencyOverrides {
                fixed_latency_ms: Some(500),
                ..LatencyOverrides::default()
            },
        );
        assert_eq!(got.adaptive_min_ms, 500);
        assert_eq!(got.adaptive_max_ms, 500);
        assert_eq!(got.prefill_ms, 400, "prefill is fixed.min(400)");
    }

    #[test]
    fn broadcast_override_is_clamped() {
        let low = resolve(
            "broadcast",
            false,
            LatencyOverrides {
                fixed_latency_ms: Some(1),
                ..LatencyOverrides::default()
            },
        );
        assert_eq!(low.adaptive_min_ms, FIXED_LATENCY_MIN_MS);
        let high = resolve(
            "broadcast",
            false,
            LatencyOverrides {
                fixed_latency_ms: Some(99_000),
                ..LatencyOverrides::default()
            },
        );
        assert_eq!(high.adaptive_min_ms, FIXED_LATENCY_MAX_MS);
    }

    #[test]
    fn broadcast_override_never_shrinks_profile_ring() {
        // A small fixed target must not drop the loopback variant's 4000ms floor.
        let got = resolve(
            "broadcast",
            true,
            LatencyOverrides {
                fixed_latency_ms: Some(100),
                ..LatencyOverrides::default()
            },
        );
        assert_eq!(got.ring_ms, params("broadcast", true).ring_ms);
        assert!(got.ring_ms >= got.adaptive_max_ms);
    }

    #[test]
    fn broadcast_large_override_grows_ring_for_headroom() {
        let got = resolve(
            "broadcast",
            false,
            LatencyOverrides {
                fixed_latency_ms: Some(2000),
                ..LatencyOverrides::default()
            },
        );
        assert_eq!(got.ring_ms, 8000, "ring is max(profile ring, fixed * 4)");
        assert!(got.ring_ms >= got.adaptive_max_ms);
    }

    #[test]
    fn broadcast_without_override_uses_profile_defaults() {
        assert_eq!(
            resolve("broadcast", false, LatencyOverrides::default()),
            params("broadcast", false)
        );
    }
}
