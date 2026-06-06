//! Honest streaming metrics: real glitch counters, quality scoring, and
//! best-effort TCP round-trip time. Replaces the previous score derived from
//! socket `write()` timing.

/// A best-effort RTT reading from the OS TCP stack.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RttSample {
    pub srtt_ms: f32,
    pub rttvar_ms: f32,
}

/// Computes a 0-100 stream-quality score (higher is better) from real signals.
///
/// - `glitches_delta`: underruns + overruns observed in the last window — the
///   only thing the listener actually hears; weighted heaviest.
/// - `occupancy_ratio`: buffer fill 0.0..=1.0 — high occupancy means latency.
/// - `rtt_ms`: best-effort network RTT, or `None` when unavailable.
pub fn quality_score(glitches_delta: u64, occupancy_ratio: f32, rtt_ms: Option<f32>) -> u8 {
    let mut score: i32 = 100;

    // Audible glitches: each one in the window is a large penalty (cap the hit).
    score -= (glitches_delta.min(10) as i32) * 8; // up to -80

    // Latency from over-buffering (only the half above 50% counts).
    let occ = occupancy_ratio.clamp(0.0, 1.0);
    if occ > 0.5 {
        score -= ((occ - 0.5) * 2.0 * 20.0) as i32; // up to -20
    }

    // Network RTT above 50 ms erodes the score, capped.
    if let Some(rtt) = rtt_ms {
        if rtt > 50.0 {
            score -= ((rtt - 50.0) / 10.0).min(20.0) as i32; // up to -20
        }
    }

    score.clamp(0, 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_stream_scores_100() {
        assert_eq!(quality_score(0, 0.1, Some(5.0)), 100);
        assert_eq!(quality_score(0, 0.1, None), 100);
    }

    #[test]
    fn glitches_dominate_the_penalty() {
        assert_eq!(quality_score(1, 0.0, None), 92);
        assert_eq!(quality_score(5, 0.0, None), 60);
        assert_eq!(quality_score(100, 0.0, None), 20); // capped at 10 glitches => -80
    }

    #[test]
    fn high_occupancy_adds_latency_penalty() {
        // occupancy 1.0 => (1.0-0.5)*2*20 = 20 penalty
        assert_eq!(quality_score(0, 1.0, None), 80);
    }

    #[test]
    fn high_rtt_erodes_score_and_is_capped() {
        assert_eq!(quality_score(0, 0.0, Some(150.0)), 90); // (150-50)/10 = 10
        assert_eq!(quality_score(0, 0.0, Some(5000.0)), 80); // capped at 20
    }

    #[test]
    fn score_never_underflows() {
        assert_eq!(quality_score(100, 1.0, Some(5000.0)), 0);
    }
}
