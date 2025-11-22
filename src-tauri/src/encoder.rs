use flac_bound::{FlacEncoder, WriteWrapper};
use opus::Encoder as OpusEncoder;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    None = 0,
    Flac = 1,
    Opus = 2,
}

impl From<u8> for Codec {
    fn from(byte: u8) -> Self {
        match byte {
            1 => Codec::Flac,
            2 => Codec::Opus,
            _ => Codec::None,
        }
    }
}

pub enum AudioEncoder {
    None,
    Flac,
    Opus(Arc<Mutex<OpusEncoder>>),
}

impl AudioEncoder {
    pub fn new(codec: Codec, sample_rate: u32, _channels: u16) -> Result<Self, String> {
        match codec {
            Codec::None => Ok(AudioEncoder::None),
            Codec::Flac => {
                // We use stateless encoding for FLAC (per chunk) to avoid lifetime issues
                // with the underlying C library bindings.
                Ok(AudioEncoder::Flac)
            }
            Codec::Opus => {
                let encoder = OpusEncoder::new(
                    sample_rate,
                    opus::Channels::Stereo,
                    opus::Application::Audio,
                )
                .map_err(|e| format!("Failed to create Opus encoder: {}", e))?;
                Ok(AudioEncoder::Opus(Arc::new(Mutex::new(encoder))))
            }
        }
    }

    pub fn encode(&self, pcm_data: &[u8], _channels: u16) -> Result<Vec<u8>, String> {
        match self {
            AudioEncoder::None => Ok(pcm_data.to_vec()),
            AudioEncoder::Flac => {
                // This path is unused because audio.rs calls encode_flac_chunk directly
                Err("Use encode_flac_chunk for FLAC".to_string())
            }
            AudioEncoder::Opus(encoder_mutex) => {
                let mut encoder = encoder_mutex.lock().unwrap();

                // Convert u8 bytes to i16 samples
                let samples: Vec<i16> = pcm_data
                    .chunks_exact(2)
                    .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();

                let mut output = vec![0u8; pcm_data.len()];
                let len = encoder
                    .encode(&samples, &mut output)
                    .map_err(|e| format!("Opus encode error: {}", e))?;

                output.truncate(len);
                Ok(output)
            }
        }
    }

    pub fn encode_flac_chunk(
        pcm_data: &[u8],
        sample_rate: u32,
        channels: u16,
    ) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        {
            let mut wrapper = WriteWrapper(&mut out);
            let mut encoder = FlacEncoder::new()
                .ok_or("Failed to create FLAC encoder config")?
                .channels(channels as u32)
                .bits_per_sample(16)
                .sample_rate(sample_rate)
                .init_write(&mut wrapper)
                .map_err(|e| format!("FLAC init error: {:?}", e))?;

            let samples: Vec<i32> = pcm_data
                .chunks_exact(2)
                .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as i32)
                .collect();

            encoder
                .process_interleaved(&samples, samples.len() as u32 / channels as u32)
                .map_err(|e| format!("FLAC process error: {:?}", e))?;

            encoder
                .finish()
                .map_err(|e| format!("FLAC finish error: {:?}", e))?;
        }
        Ok(out)
    }

    pub fn get_codec_byte(&self) -> u8 {
        match self {
            AudioEncoder::None => 0,
            AudioEncoder::Flac => 1,
            AudioEncoder::Opus(_) => 2,
        }
    }

    pub fn set_bitrate(&self, bitrate: i32) -> Result<(), String> {
        match self {
            AudioEncoder::Opus(encoder_mutex) => {
                let mut encoder = encoder_mutex.lock().map_err(|e| e.to_string())?;
                encoder
                    .set_bitrate(opus::Bitrate::Bits(bitrate))
                    .map_err(|e| format!("Failed to set bitrate: {}", e))?;
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
