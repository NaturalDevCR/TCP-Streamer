//! Real-time-safe cpal capture: the audio callback holds no lock and performs
//! no heap allocation after warmup.
//!
//! The callback normalizes every device layout to INTERLEAVED STEREO f32
//! before pushing into the ring: mono is duplicated, stereo passes through,
//! and for >2 channels the front pair (FL/FR) is taken — converting only the
//! samples that are kept, so a 16-channel device costs no more than stereo.
//! The ring therefore always holds stereo at the capture rate, which keeps
//! every downstream ms↔samples computation channel-agnostic.

use cpal::traits::DeviceTrait;
use cpal::{BufferSize, SampleFormat, SupportedBufferSize};
use ringbuf::HeapProducer;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Converts one callback buffer to interleaved stereo f32 in `scratch` and
/// pushes it into `producer`. Samples that don't fit are counted as overruns.
/// A trailing partial frame is ignored. With the ring capacity kept even, a
/// partial push can never split a stereo frame (free space stays even).
/// `pub(crate)` so the engine's pipeline test can drive it like cpal would.
#[inline]
pub(crate) fn push_stereo<T: Copy>(
    data: &[T],
    in_channels: usize,
    conv: impl Fn(T) -> f32,
    scratch: &mut Vec<f32>,
    producer: &mut HeapProducer<f32>,
    overruns: &AtomicU64,
) {
    scratch.clear();
    match in_channels {
        0 => return,
        1 => {
            scratch.reserve(data.len() * 2);
            for &s in data {
                let v = conv(s);
                scratch.push(v);
                scratch.push(v);
            }
        }
        n => {
            let frames = data.len() / n;
            scratch.reserve(frames * 2);
            for f in 0..frames {
                scratch.push(conv(data[f * n]));
                scratch.push(conv(data[f * n + 1]));
            }
        }
    }
    let pushed = producer.push_slice(scratch);
    if pushed < scratch.len() {
        overruns.fetch_add((scratch.len() - pushed) as u64, Ordering::Relaxed);
    }
}

/// Builds an input stream for the given format, moving `producer` into the
/// callback. `in_channels` is the device's native channel count (as opened);
/// the ring receives interleaved stereo f32 regardless. `overruns` counts
/// samples dropped when the ring cannot accept a whole callback buffer.
pub fn build_input_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: SampleFormat,
    in_channels: u16,
    mut producer: HeapProducer<f32>,
    overruns: Arc<AtomicU64>,
    capacity_hint: usize,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let err_fn = |err: cpal::StreamError| {
        log::error!("CPAL stream error: {}", err);
    };
    let ch = in_channels as usize;
    // Generous floor so even unusually large OS callback buffers fit without
    // a mid-stream reallocation (16k stereo samples ≈ 170ms @ 48kHz).
    let mut scratch: Vec<f32> = Vec::with_capacity(capacity_hint.max(16_384));

    match format {
        SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                push_stereo(data, ch, |s| s, &mut scratch, &mut producer, &overruns);
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                push_stereo(
                    data,
                    ch,
                    |s| s as f32 / 32768.0,
                    &mut scratch,
                    &mut producer,
                    &overruns,
                );
            },
            err_fn,
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            config,
            move |data: &[u16], _: &cpal::InputCallbackInfo| {
                push_stereo(
                    data,
                    ch,
                    |s| (s as f32 - 32768.0) / 32768.0,
                    &mut scratch,
                    &mut producer,
                    &overruns,
                );
            },
            err_fn,
            None,
        ),
        SampleFormat::I32 => device.build_input_stream(
            config,
            move |data: &[i32], _: &cpal::InputCallbackInfo| {
                push_stereo(
                    data,
                    ch,
                    |s| s as f32 / 2_147_483_648.0,
                    &mut scratch,
                    &mut producer,
                    &overruns,
                );
            },
            err_fn,
            None,
        ),
        other => {
            log::error!("Unsupported sample format: {:?}", other);
            Err(cpal::BuildStreamError::StreamConfigNotSupported)
        }
    }
}

/// Resolves a requested hardware buffer size (in frames) against the device's
/// supported range. Out-of-range requests are clamped; `Unknown` ranges and a
/// zero/unset request fall back to the driver default.
#[allow(dead_code)]
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
mod push_stereo_tests {
    use super::*;
    use ringbuf::HeapRb;

    fn run_push<T: Copy>(
        data: &[T],
        in_channels: usize,
        conv: impl Fn(T) -> f32,
        ring_capacity: usize,
    ) -> (Vec<f32>, u64) {
        let rb = HeapRb::<f32>::new(ring_capacity);
        let (mut prod, mut cons) = rb.split();
        let overruns = AtomicU64::new(0);
        let mut scratch = Vec::new();
        push_stereo(data, in_channels, conv, &mut scratch, &mut prod, &overruns);
        let mut out = vec![0.0f32; cons.len()];
        let n = cons.pop_slice(&mut out);
        out.truncate(n);
        (out, overruns.load(Ordering::Relaxed))
    }

    #[test]
    fn mono_is_duplicated_to_both_channels() {
        let (out, over) = run_push(&[0.1f32, -0.2, 0.3], 1, |s| s, 64);
        assert_eq!(out, vec![0.1, 0.1, -0.2, -0.2, 0.3, 0.3]);
        assert_eq!(over, 0);
    }

    #[test]
    fn stereo_passes_through() {
        let (out, _) = run_push(&[0.1f32, -0.1, 0.2, -0.2], 2, |s| s, 64);
        assert_eq!(out, vec![0.1, -0.1, 0.2, -0.2]);
    }

    #[test]
    fn six_channels_keep_front_pair_only() {
        // Two 6ch frames: (L,R,C,LFE,SL,SR)
        let data = [
            0.1f32, -0.1, 9.0, 9.0, 9.0, 9.0, //
            0.2, -0.2, 9.0, 9.0, 9.0, 9.0,
        ];
        let (out, _) = run_push(&data, 6, |s| s, 64);
        assert_eq!(out, vec![0.1, -0.1, 0.2, -0.2]);
    }

    #[test]
    fn trailing_partial_frame_is_ignored() {
        let data = [0.1f32, -0.1, 0.5]; // 1.5 stereo frames
        let (out, _) = run_push(&data, 2, |s| s, 64);
        assert_eq!(out, vec![0.1, -0.1]);
    }

    #[test]
    fn i16_full_scale_converts_close_to_unit() {
        let (out, _) = run_push(&[i16::MAX, i16::MIN], 2, |s| s as f32 / 32768.0, 64);
        assert!((out[0] - 0.99997).abs() < 1e-4);
        assert!((out[1] + 1.0).abs() < 1e-6);
    }

    #[test]
    fn i32_full_scale_converts_close_to_unit() {
        let (out, _) = run_push(&[i32::MAX, i32::MIN], 2, |s| s as f32 / 2_147_483_648.0, 64);
        assert!((out[0] - 1.0).abs() < 1e-6);
        assert!((out[1] + 1.0).abs() < 1e-6);
    }

    #[test]
    fn u16_midpoint_is_silence() {
        let (out, _) = run_push(
            &[32768u16, 0, 65535],
            1,
            |s| (s as f32 - 32768.0) / 32768.0,
            64,
        );
        assert!(out[0].abs() < 1e-6 && out[1].abs() < 1e-6);
        assert!((out[2] + 1.0).abs() < 1e-3);
        assert!((out[4] - 0.99997).abs() < 1e-4);
    }

    #[test]
    fn overruns_counted_and_frames_never_split_with_even_ring() {
        // Ring of 4 samples (2 stereo frames), push 3 frames → 1 frame dropped.
        let (out, over) = run_push(&[1.0f32, -1.0, 2.0, -2.0, 3.0, -3.0], 2, |s| s, 4);
        assert_eq!(out, vec![1.0, -1.0, 2.0, -2.0]);
        assert_eq!(over, 2); // two samples (one whole frame) dropped
    }

    #[test]
    fn zero_channels_is_a_noop() {
        let (out, over) = run_push(&[1.0f32, 2.0], 0, |s| s, 64);
        assert!(out.is_empty());
        assert_eq!(over, 0);
    }
}

/// Hardware smoke test (`cargo test -- --ignored`): negotiates a config for
/// the default input device exactly like the engine does and verifies the OS
/// accepts it (stream builds and plays). Capture content is not asserted —
/// that depends on mic permission — only that the negotiated config is real.
#[cfg(test)]
mod hardware_smoke {
    use super::*;
    use crate::audio::engine::device::{negotiate, ConfigCandidate, SampleFmt};
    use cpal::traits::{HostTrait, StreamTrait};
    use ringbuf::HeapRb;

    #[test]
    #[ignore = "requires a real audio input device"]
    fn default_device_accepts_negotiated_config() {
        let host = cpal::default_host();
        let device = host.default_input_device().expect("no input device");
        let supported: Vec<_> = device
            .supported_input_configs()
            .expect("query configs")
            .collect();
        let to_fmt = |f: SampleFormat| match f {
            SampleFormat::F32 => Some(SampleFmt::F32),
            SampleFormat::I16 => Some(SampleFmt::I16),
            SampleFormat::U16 => Some(SampleFmt::U16),
            SampleFormat::I32 => Some(SampleFmt::I32),
            _ => None,
        };
        let usable: Vec<(ConfigCandidate, &cpal::SupportedStreamConfigRange)> = supported
            .iter()
            .filter_map(|r| {
                to_fmt(r.sample_format()).map(|format| {
                    (
                        ConfigCandidate {
                            channels: r.channels(),
                            format,
                            min_rate: r.min_sample_rate().0,
                            max_rate: r.max_sample_rate().0,
                        },
                        r,
                    )
                })
            })
            .collect();
        let candidates: Vec<ConfigCandidate> = usable.iter().map(|(c, _)| *c).collect();
        let n = negotiate(&candidates, 48_000).expect("no usable config");
        let range = usable[n.index].1;
        let config = cpal::StreamConfig {
            channels: range.channels(),
            sample_rate: cpal::SampleRate(n.capture_rate),
            buffer_size: cpal::BufferSize::Default,
        };
        println!(
            "negotiated: {:?} {}ch @ {}Hz (requested 48000)",
            range.sample_format(),
            range.channels(),
            n.capture_rate
        );

        let rb = HeapRb::<f32>::new(48_000 * 2);
        let (prod, cons) = rb.split();
        let overruns = Arc::new(AtomicU64::new(0));
        let stream = build_input_stream(
            &device,
            &config,
            range.sample_format(),
            range.channels(),
            prod,
            overruns,
            4096,
        )
        .expect("negotiated config must build");
        stream.play().expect("stream must start");
        std::thread::sleep(std::time::Duration::from_millis(800));
        let captured = cons.len();
        println!("captured {captured} stereo samples in 800ms");
        assert_eq!(captured % 2, 0, "ring must hold whole stereo frames");
        drop(stream);
    }
}

#[cfg(test)]
mod buffer_size_tests {
    use super::*;

    #[test]
    fn within_range_is_used_verbatim() {
        let s = SupportedBufferSize::Range { min: 64, max: 4096 };
        assert!(matches!(
            resolve_buffer_size(1024, &s),
            BufferSize::Fixed(1024)
        ));
    }

    #[test]
    fn below_min_clamps_up() {
        let s = SupportedBufferSize::Range {
            min: 256,
            max: 4096,
        };
        assert!(matches!(
            resolve_buffer_size(64, &s),
            BufferSize::Fixed(256)
        ));
    }

    #[test]
    fn above_max_clamps_down() {
        let s = SupportedBufferSize::Range { min: 64, max: 2048 };
        assert!(matches!(
            resolve_buffer_size(8192, &s),
            BufferSize::Fixed(2048)
        ));
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
