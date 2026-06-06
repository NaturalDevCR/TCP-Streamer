//! Real-time-safe cpal capture: the audio callback holds no lock and performs
//! no heap allocation after warmup.

use cpal::traits::DeviceTrait;
use cpal::{BufferSize, SampleFormat, SupportedBufferSize};
use ringbuf::HeapProducer;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Builds an input stream for the given format, moving `producer` into the
/// callback. `overruns` is incremented by the number of samples dropped when
/// the ring buffer cannot accept the whole callback buffer.
pub fn build_input_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: SampleFormat,
    mut producer: HeapProducer<f32>,
    overruns: Arc<AtomicU64>,
    capacity_hint: usize,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let err_fn = |err: cpal::StreamError| {
        log::error!("CPAL stream error: {}", err);
    };

    match format {
        SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let pushed = producer.push_slice(data);
                if pushed < data.len() {
                    overruns.fetch_add((data.len() - pushed) as u64, Ordering::Relaxed);
                }
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => {
            let mut scratch: Vec<f32> = Vec::with_capacity(capacity_hint);
            device.build_input_stream(
                config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    scratch.clear();
                    scratch.extend(data.iter().map(|&s| s as f32 / 32768.0));
                    let pushed = producer.push_slice(&scratch);
                    if pushed < scratch.len() {
                        overruns.fetch_add((scratch.len() - pushed) as u64, Ordering::Relaxed);
                    }
                },
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let mut scratch: Vec<f32> = Vec::with_capacity(capacity_hint);
            device.build_input_stream(
                config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    scratch.clear();
                    scratch.extend(data.iter().map(|&s| (s as f32 - 32768.0) / 32768.0));
                    let pushed = producer.push_slice(&scratch);
                    if pushed < scratch.len() {
                        overruns.fetch_add((scratch.len() - pushed) as u64, Ordering::Relaxed);
                    }
                },
                err_fn,
                None,
            )
        }
        other => {
            log::error!("Unsupported sample format: {:?}", other);
            Err(cpal::BuildStreamError::StreamConfigNotSupported)
        }
    }
}

/// Resolves a requested hardware buffer size (in frames) against the device's
/// supported range. Out-of-range requests are clamped; `Unknown` ranges and a
/// zero/unset request fall back to the driver default.
pub fn resolve_buffer_size(requested: u32, supported: &SupportedBufferSize) -> BufferSize {
    match supported {
        SupportedBufferSize::Range { min, max } => {
            if requested == 0 {
                BufferSize::Default
            } else {
                BufferSize::Fixed(requested.clamp(*min, *max))
            }
        }
        SupportedBufferSize::Unknown => BufferSize::Default,
    }
}

#[cfg(test)]
mod buffer_size_tests {
    use super::*;

    #[test]
    fn within_range_is_used_verbatim() {
        let s = SupportedBufferSize::Range { min: 64, max: 4096 };
        assert!(matches!(resolve_buffer_size(1024, &s), BufferSize::Fixed(1024)));
    }

    #[test]
    fn below_min_clamps_up() {
        let s = SupportedBufferSize::Range { min: 256, max: 4096 };
        assert!(matches!(resolve_buffer_size(64, &s), BufferSize::Fixed(256)));
    }

    #[test]
    fn above_max_clamps_down() {
        let s = SupportedBufferSize::Range { min: 64, max: 2048 };
        assert!(matches!(resolve_buffer_size(8192, &s), BufferSize::Fixed(2048)));
    }

    #[test]
    fn unknown_range_falls_back_to_default() {
        assert!(matches!(
            resolve_buffer_size(1024, &SupportedBufferSize::Unknown),
            BufferSize::Default
        ));
    }

    #[test]
    fn zero_request_falls_back_to_default() {
        let s = SupportedBufferSize::Range { min: 64, max: 4096 };
        assert!(matches!(resolve_buffer_size(0, &s), BufferSize::Default));
    }
}
