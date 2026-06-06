//! Encoding of f32 audio samples into wire formats.

/// Encodes f32 samples (nominally in [-1.0, 1.0]) into signed 16-bit
/// little-endian PCM, replacing the contents of `out`. Out-of-range samples
/// are clamped before scaling. `out` keeps its capacity across calls, so a
/// reused buffer performs no allocation after warmup.
pub fn encode_f32_to_pcm_i16_le(samples: &[f32], out: &mut Vec<u8>) {
    out.clear();
    out.reserve(samples.len() * 2);
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let v = (clamped * 32767.0) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_known_values_little_endian() {
        let mut out = Vec::new();
        encode_f32_to_pcm_i16_le(&[0.0, 1.0, -1.0], &mut out);
        // 0 -> 0x0000, 1.0 -> 32767 (0x7FFF), -1.0 -> -32767 (0x8001)
        assert_eq!(out, vec![0x00, 0x00, 0xFF, 0x7F, 0x01, 0x80]);
    }

    #[test]
    fn clamps_out_of_range_input() {
        let mut out = Vec::new();
        encode_f32_to_pcm_i16_le(&[2.0, -2.0], &mut out);
        // clamps to 1.0 -> 32767 and -1.0 -> -32767
        assert_eq!(out, vec![0xFF, 0x7F, 0x01, 0x80]);
    }

    #[test]
    fn empty_input_yields_empty_output() {
        let mut out = vec![1, 2, 3];
        encode_f32_to_pcm_i16_le(&[], &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn output_len_is_two_bytes_per_sample() {
        let mut out = Vec::new();
        encode_f32_to_pcm_i16_le(&[0.1, 0.2, 0.3, 0.4], &mut out);
        assert_eq!(out.len(), 8);
    }
}
