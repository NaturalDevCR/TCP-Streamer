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
