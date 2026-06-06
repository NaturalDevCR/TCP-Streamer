//! Pure audio-config selection logic, decoupled from cpal so it can be tested.

/// Sample format candidates we support, in nothing-implied order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFmt {
    F32,
    I16,
    U16,
}

/// A simplified view of a `cpal::SupportedStreamConfigRange`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigCandidate {
    pub channels: u16,
    pub format: SampleFmt,
    pub min_rate: u32,
    pub max_rate: u32,
}

/// Picks the index of the best candidate for the requested sample rate.
///
/// Preference: stereo (2ch) before mono (1ch); within a channel count,
/// F32 before I16 before U16. Only candidates whose [min_rate, max_rate]
/// range contains `sample_rate` are eligible. Returns `None` if none match.
pub fn pick_best(candidates: &[ConfigCandidate], sample_rate: u32) -> Option<usize> {
    for ch in [2u16, 1u16] {
        for fmt in [SampleFmt::F32, SampleFmt::I16, SampleFmt::U16] {
            for (i, c) in candidates.iter().enumerate() {
                if c.channels == ch
                    && c.format == fmt
                    && c.min_rate <= sample_rate
                    && c.max_rate >= sample_rate
                {
                    return Some(i);
                }
            }
        }
    }
    None
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
    fn prefers_stereo_f32_when_available() {
        let cands = vec![
            c(1, SampleFmt::F32, 8000, 48000),
            c(2, SampleFmt::I16, 8000, 48000),
            c(2, SampleFmt::F32, 8000, 48000),
        ];
        assert_eq!(pick_best(&cands, 48000), Some(2));
    }

    #[test]
    fn falls_back_to_mono_when_no_stereo() {
        let cands = vec![c(1, SampleFmt::I16, 8000, 48000)];
        assert_eq!(pick_best(&cands, 48000), Some(0));
    }

    #[test]
    fn respects_sample_rate_range() {
        let cands = vec![
            c(2, SampleFmt::F32, 8000, 44100), // does not cover 48000
            c(2, SampleFmt::I16, 8000, 48000),
        ];
        assert_eq!(pick_best(&cands, 48000), Some(1));
    }

    #[test]
    fn returns_none_when_nothing_matches() {
        let cands = vec![c(2, SampleFmt::F32, 96000, 192000)];
        assert_eq!(pick_best(&cands, 48000), None);
    }
}
