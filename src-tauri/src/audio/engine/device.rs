//! Pure audio-config selection logic, decoupled from cpal so it can be tested.
//!
//! The engine normalizes everything to a fixed wire format (stereo s16le at
//! the configured output rate), so negotiation no longer needs the device to
//! match that format: any channel count works (mixed to stereo) and any
//! supported rate works (resampled internally). Preference order favors the
//! cheapest conversion: native stereo, then multichannel (front pair taken),
//! then mono; exact rate before resampled; float before integer formats.

/// Sample format candidates we support, in nothing-implied order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFmt {
    F32,
    I16,
    U16,
    I32,
}

/// A simplified view of a `cpal::SupportedStreamConfigRange`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigCandidate {
    pub channels: u16,
    pub format: SampleFmt,
    pub min_rate: u32,
    pub max_rate: u32,
}

/// Result of [`negotiate`]: which candidate to open and at what rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Negotiated {
    pub index: usize,
    /// Rate to open the device with — always inside the candidate's supported
    /// range. May differ from the requested rate; the engine resamples.
    pub capture_rate: u32,
}

/// Channel preference: stereo is free, extra channels are dropped (front pair
/// kept) so fewer extras are better, and mono loses stereo so it ranks last.
fn channel_score(ch: u16) -> u32 {
    match ch {
        2 => 0,
        0 => u32::MAX,
        1 => 10_000,
        n => u32::from(n) - 2,
    }
}

/// Format preference: f32 is lossless for our pipeline, then by how cheaply
/// and faithfully the format converts to f32.
fn format_score(f: SampleFmt) -> u32 {
    match f {
        SampleFmt::F32 => 0,
        SampleFmt::I16 => 1,
        SampleFmt::I32 => 2,
        SampleFmt::U16 => 3,
    }
}

/// Picks the best candidate for the requested rate and the concrete rate to
/// open it with (the requested rate when supported, otherwise the closest
/// rate inside the candidate's range). Returns `None` only when `candidates`
/// is empty.
pub fn negotiate(candidates: &[ConfigCandidate], requested_rate: u32) -> Option<Negotiated> {
    candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| c.channels > 0 && c.min_rate <= c.max_rate)
        .map(|(i, c)| {
            let capture_rate = requested_rate.clamp(c.min_rate, c.max_rate);
            let rate_distance = capture_rate.abs_diff(requested_rate);
            let key = (
                channel_score(c.channels),
                rate_distance,
                format_score(c.format),
                i,
            );
            (key, i, capture_rate)
        })
        .min_by_key(|(key, _, _)| *key)
        .map(|(_, index, capture_rate)| Negotiated {
            index,
            capture_rate,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(channels: u16, format: SampleFmt, min_rate: u32, max_rate: u32) -> ConfigCandidate {
        ConfigCandidate {
            channels,
            format,
            min_rate,
            max_rate,
        }
    }

    #[test]
    fn prefers_stereo_f32_at_exact_rate() {
        let cands = vec![
            c(1, SampleFmt::F32, 8000, 48000),
            c(2, SampleFmt::I16, 8000, 48000),
            c(2, SampleFmt::F32, 8000, 48000),
        ];
        assert_eq!(
            negotiate(&cands, 48000),
            Some(Negotiated {
                index: 2,
                capture_rate: 48000
            })
        );
    }

    #[test]
    fn multichannel_beats_mono_when_no_stereo() {
        // A 16ch loopback device still carries true L/R in its front pair;
        // mono loses stereo entirely.
        let cands = vec![
            c(1, SampleFmt::F32, 8000, 48000),
            c(16, SampleFmt::F32, 8000, 48000),
        ];
        assert_eq!(
            negotiate(&cands, 48000),
            Some(Negotiated {
                index: 1,
                capture_rate: 48000
            })
        );
    }

    #[test]
    fn fewer_extra_channels_preferred() {
        let cands = vec![
            c(16, SampleFmt::F32, 8000, 48000),
            c(6, SampleFmt::F32, 8000, 48000),
        ];
        assert_eq!(negotiate(&cands, 48000).map(|n| n.index), Some(1));
    }

    #[test]
    fn mono_only_device_is_accepted() {
        let cands = vec![c(1, SampleFmt::I16, 8000, 48000)];
        assert_eq!(
            negotiate(&cands, 48000),
            Some(Negotiated {
                index: 0,
                capture_rate: 48000
            })
        );
    }

    #[test]
    fn unsupported_rate_clamps_to_range() {
        // Device only does 44.1k: open at 44.1k; the engine resamples to 48k.
        let cands = vec![c(2, SampleFmt::F32, 44100, 44100)];
        assert_eq!(
            negotiate(&cands, 48000),
            Some(Negotiated {
                index: 0,
                capture_rate: 44100
            })
        );
    }

    #[test]
    fn closest_rate_wins_among_equal_channels() {
        let cands = vec![
            c(2, SampleFmt::F32, 96000, 96000),
            c(2, SampleFmt::F32, 8000, 44100),
        ];
        // |44100 - 48000| = 3900 < |96000 - 48000| = 48000.
        assert_eq!(
            negotiate(&cands, 48000),
            Some(Negotiated {
                index: 1,
                capture_rate: 44100
            })
        );
    }

    #[test]
    fn exact_rate_beats_resampled_even_with_worse_format() {
        let cands = vec![
            c(2, SampleFmt::F32, 44100, 44100),
            c(2, SampleFmt::U16, 48000, 48000),
        ];
        assert_eq!(
            negotiate(&cands, 48000),
            Some(Negotiated {
                index: 1,
                capture_rate: 48000
            })
        );
    }

    #[test]
    fn format_preference_breaks_ties() {
        let cands = vec![
            c(2, SampleFmt::U16, 8000, 48000),
            c(2, SampleFmt::I32, 8000, 48000),
            c(2, SampleFmt::I16, 8000, 48000),
            c(2, SampleFmt::F32, 8000, 48000),
        ];
        assert_eq!(negotiate(&cands, 48000).map(|n| n.index), Some(3));
    }

    #[test]
    fn stereo_with_resample_beats_mono_at_exact_rate() {
        let cands = vec![
            c(1, SampleFmt::F32, 48000, 48000),
            c(2, SampleFmt::F32, 44100, 44100),
        ];
        assert_eq!(
            negotiate(&cands, 48000),
            Some(Negotiated {
                index: 1,
                capture_rate: 44100
            })
        );
    }

    #[test]
    fn empty_candidates_yield_none() {
        assert_eq!(negotiate(&[], 48000), None);
    }

    #[test]
    fn zero_channel_or_inverted_range_candidates_are_ignored() {
        let cands = vec![
            c(0, SampleFmt::F32, 8000, 48000),
            c(2, SampleFmt::F32, 48000, 8000), // inverted range
            c(1, SampleFmt::I16, 8000, 48000),
        ];
        assert_eq!(negotiate(&cands, 48000).map(|n| n.index), Some(2));
    }
}
