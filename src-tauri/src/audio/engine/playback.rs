//! Real-time-safe cpal output playback. The callback pulls f32 samples from a
//! lock-free ring consumer; underruns are filled with silence. The ring holds
//! samples already converted to the DEVICE's rate and channel count; this
//! module only converts the sample TYPE (f32 → whatever the device wants).

use cpal::traits::DeviceTrait;
use cpal::{SampleFormat, StreamConfig};
use ringbuf::HeapConsumer;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Pops up to `data.len()` f32 samples and writes them through `conv`;
/// missing samples are written as `silence` and counted in `underruns`.
#[inline]
fn fill_output<T: Copy>(
    data: &mut [T],
    conv: impl Fn(f32) -> T,
    silence: T,
    scratch: &mut Vec<f32>,
    consumer: &mut HeapConsumer<f32>,
    underruns: &AtomicU64,
) {
    scratch.clear();
    scratch.resize(data.len(), 0.0);
    let popped = consumer.pop_slice(scratch);
    for (dst, &src) in data.iter_mut().zip(scratch[..popped].iter()) {
        *dst = conv(src);
    }
    if popped < data.len() {
        for dst in &mut data[popped..] {
            *dst = silence;
        }
        underruns.fetch_add((data.len() - popped) as u64, Ordering::Relaxed);
    }
}

/// Builds an output stream in the device's negotiated sample format, draining
/// f32 samples from `consumer`.
pub fn build_output_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    format: SampleFormat,
    mut consumer: HeapConsumer<f32>,
    underruns: Arc<AtomicU64>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let err_fn = |err: cpal::StreamError| log::error!("CPAL output error: {}", err);
    // Preallocated so the integer-format callbacks never allocate mid-stream
    // (resize within capacity is a plain fill).
    let mut scratch: Vec<f32> = Vec::with_capacity(16_384);

    match format {
        SampleFormat::F32 => device.build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let popped = consumer.pop_slice(data);
                if popped < data.len() {
                    for s in &mut data[popped..] {
                        *s = 0.0;
                    }
                    underruns.fetch_add((data.len() - popped) as u64, Ordering::Relaxed);
                }
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_output_stream(
            config,
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                fill_output(
                    data,
                    |s| (s.clamp(-1.0, 1.0) * 32767.0) as i16,
                    0,
                    &mut scratch,
                    &mut consumer,
                    &underruns,
                );
            },
            err_fn,
            None,
        ),
        SampleFormat::U16 => device.build_output_stream(
            config,
            move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                fill_output(
                    data,
                    |s| (s.clamp(-1.0, 1.0) * 32767.0 + 32768.0) as u16,
                    32768,
                    &mut scratch,
                    &mut consumer,
                    &underruns,
                );
            },
            err_fn,
            None,
        ),
        SampleFormat::I32 => device.build_output_stream(
            config,
            move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                fill_output(
                    data,
                    |s| (s.clamp(-1.0, 1.0) as f64 * 2_147_483_647.0) as i32,
                    0,
                    &mut scratch,
                    &mut consumer,
                    &underruns,
                );
            },
            err_fn,
            None,
        ),
        other => {
            log::error!("Unsupported output sample format: {:?}", other);
            Err(cpal::BuildStreamError::StreamConfigNotSupported)
        }
    }
}
