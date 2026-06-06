//! Real-time-safe cpal output playback. The callback pulls f32 samples from a
//! lock-free ring consumer; underruns are filled with silence.

use cpal::traits::DeviceTrait;
use cpal::StreamConfig;
use ringbuf::HeapConsumer;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Builds an output stream that drains f32 samples from `consumer`. Missing
/// samples (underrun) are written as silence and counted in `underruns`.
pub fn build_output_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    mut consumer: HeapConsumer<f32>,
    underruns: Arc<AtomicU64>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let err_fn = |err: cpal::StreamError| log::error!("CPAL output error: {}", err);
    device.build_output_stream(
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
    )
}
