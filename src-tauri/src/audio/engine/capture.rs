//! Real-time-safe cpal capture: the audio callback holds no lock and performs
//! no heap allocation after warmup.

use cpal::traits::DeviceTrait;
use cpal::SampleFormat;
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
