//! Decoding of signed 16-bit little-endian PCM into f32 samples. Inverse of
//! `engine::encoder::encode_f32_to_pcm_i16_le`.

/// Decodes i16 LE PCM bytes into f32 samples in [-1.0, 1.0], replacing `out`.
/// A trailing odd byte (incomplete sample) is ignored.
pub fn decode_pcm_i16_le_to_f32(bytes: &[u8], out: &mut Vec<f32>) {
    out.clear();
    out.reserve(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let v = i16::from_le_bytes([pair[0], pair[1]]);
        out.push(v as f32 / 32768.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::engine::encoder::encode_f32_to_pcm_i16_le;

    #[test]
    fn decodes_known_values() {
        let mut out = Vec::new();
        // 0x0000 -> 0.0 ; 0x7FFF -> ~1.0 ; 0x8001 (-32767) -> ~-1.0
        decode_pcm_i16_le_to_f32(&[0x00, 0x00, 0xFF, 0x7F, 0x01, 0x80], &mut out);
        assert_eq!(out.len(), 3);
        assert!((out[0]).abs() < 1e-6);
        assert!((out[1] - 0.999).abs() < 0.01);
        assert!((out[2] + 0.999).abs() < 0.01);
    }

    #[test]
    fn ignores_trailing_odd_byte() {
        let mut out = Vec::new();
        decode_pcm_i16_le_to_f32(&[0x00, 0x00, 0x11], &mut out);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn roundtrips_with_encoder() {
        let samples = [0.0f32, 0.5, -0.5, 0.25];
        let mut pcm = Vec::new();
        encode_f32_to_pcm_i16_le(&samples, &mut pcm);
        let mut back = Vec::new();
        decode_pcm_i16_le_to_f32(&pcm, &mut back);
        assert_eq!(back.len(), samples.len());
        for (a, b) in samples.iter().zip(back.iter()) {
            assert!((a - b).abs() < 1e-3, "{a} vs {b}");
        }
    }
}
