#![deny(clippy::all)]

/// Creates a standard 44-byte WAV PCM header for audio streaming.
///
/// The data chunk size is set to 0xFFFFFFFF to indicate an unknown/streaming
/// length. This is suitable for continuous TCP streaming where the total
/// duration is unknown at start time.
pub fn create_wav_header(sample_rate: u32, channels: u16, bits_per_sample: u16) -> Vec<u8> {
    let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample) / 8;
    let block_align = channels * bits_per_sample / 8;

    let mut header = Vec::new();
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&0xffffffffu32.to_le_bytes()); // ChunkSize (unknown/max)
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size for PCM
    header.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat (1 = PCM)
    header.extend_from_slice(&channels.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&byte_rate.to_le_bytes());
    header.extend_from_slice(&block_align.to_le_bytes());
    header.extend_from_slice(&bits_per_sample.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&0xffffffffu32.to_le_bytes()); // Subchunk2Size (unknown/max)

    header
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wav_header_length() {
        let header = create_wav_header(48000, 2, 16);
        assert_eq!(header.len(), 44, "WAV header should be exactly 44 bytes");
    }

    #[test]
    fn test_wav_header_riff_marker() {
        let header = create_wav_header(44100, 1, 16);
        assert_eq!(&header[0..4], b"RIFF");
        assert_eq!(&header[8..12], b"WAVE");
        assert_eq!(&header[12..16], b"fmt ");
        assert_eq!(&header[36..40], b"data");
    }

    #[test]
    fn test_wav_header_sample_rate_encoding() {
        let header = create_wav_header(48000, 2, 16);
        let encoded_rate = u32::from_le_bytes([header[24], header[25], header[26], header[27]]);
        assert_eq!(encoded_rate, 48000);
    }

    #[test]
    fn test_wav_header_channels_encoding() {
        let header = create_wav_header(44100, 1, 16);
        let channels = u16::from_le_bytes([header[22], header[23]]);
        assert_eq!(channels, 1);
    }

    #[test]
    fn test_wav_header_byte_rate_stereo() {
        let header = create_wav_header(48000, 2, 16);
        let byte_rate = u32::from_le_bytes([header[28], header[29], header[30], header[31]]);
        assert_eq!(byte_rate, 48000 * 2 * 2); // sample_rate * channels * bytes_per_sample
    }

    #[test]
    fn test_wav_header_block_align() {
        let header = create_wav_header(48000, 2, 16);
        let block_align = u16::from_le_bytes([header[32], header[33]]);
        assert_eq!(block_align, 4); // channels * bits_per_sample / 8
    }
}
