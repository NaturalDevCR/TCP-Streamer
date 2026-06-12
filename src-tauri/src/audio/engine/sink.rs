//! Sink orchestrator: subscribe to a native source, open a cpal output stream
//! in a format the device actually supports, and pump received audio into it
//! through the conversion pipeline (channel mix + resample).

use super::super::stats::{emit_log, StreamStats};
use super::convert::SinkPipeline;
use super::device::{negotiate, ConfigCandidate, SampleFmt};
use super::playback::build_output_stream;
use cpal::traits::{DeviceTrait, HostTrait};
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::AppHandle;

#[allow(clippy::too_many_arguments)]
pub fn run_sink(
    output_device_name: String,
    source_addr: String,
    latency_profile: String,
    psk: String,
    app_handle: AppHandle,
) -> Result<(cpal::Stream, StreamStats), String> {
    // Subscribe (blocking, off the audio thread). `salt_b` must be unique per
    // subscription: it feeds AEAD key/nonce derivation, and reuse across
    // sessions with the same PSK would reuse nonces. Wall-clock nanoseconds
    // give per-session uniqueness (the salt travels in plaintext anyway).
    let salt_b: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
        ^ 0x9E37_79B9_7F4A_7C15;
    let sub =
        super::super::transport::udp::sink::subscribe(&source_addr, salt_b, Duration::from_secs(2))
            .map_err(|e| format!("subscribe failed: {e}"))?;
    let info = sub.info;
    let (key, nonce_salt) = if !psk.is_empty() {
        (
            Some(super::super::transport::udp::crypto::derive_key(
                &psk,
                info.salt_a,
                salt_b,
            )),
            super::super::transport::udp::crypto::nonce_salt(info.salt_a, salt_b),
        )
    } else {
        (None, 0)
    };
    emit_log(
        &app_handle,
        "success",
        format!(
            "Subscribed to {} ({}Hz, {}ch)",
            source_addr, info.sample_rate, info.channels
        ),
    );

    // Find the chosen output device.
    let host = cpal::default_host();
    let device = host
        .output_devices()
        .map_err(|e| e.to_string())?
        .find(|d| d.name().map(|n| n == output_device_name).unwrap_or(false))
        .ok_or_else(|| format!("Output device not found: {output_device_name}"))?;

    // Negotiate an output config the device actually supports; the pipeline
    // converts the received stream (info.*) into it. Never assume the device
    // does stereo f32 at the source's rate.
    let supported: Vec<_> = device
        .supported_output_configs()
        .map_err(|e| e.to_string())?
        .collect();
    let to_fmt = |f: cpal::SampleFormat| match f {
        cpal::SampleFormat::F32 => Some(SampleFmt::F32),
        cpal::SampleFormat::I16 => Some(SampleFmt::I16),
        cpal::SampleFormat::U16 => Some(SampleFmt::U16),
        cpal::SampleFormat::I32 => Some(SampleFmt::I32),
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
    let negotiated = negotiate(&candidates, info.sample_rate).ok_or_else(|| {
        format!(
            "No usable output config on '{}': device offers none of the \
             supported sample formats (f32/i16/u16/i32)",
            output_device_name
        )
    })?;
    let out_range = usable[negotiated.index].1;
    let out_format = out_range.sample_format();
    let out_channels = out_range.channels();
    let out_rate = negotiated.capture_rate;

    emit_log(
        &app_handle,
        "info",
        format!(
            "Sink output: {:?} {}ch @ {}Hz (stream is {}ch @ {}Hz{})",
            out_format,
            out_channels,
            out_rate,
            info.channels,
            info.sample_rate,
            if out_rate != info.sample_rate {
                ", resampling"
            } else {
                ""
            }
        ),
    );

    let config = cpal::StreamConfig {
        channels: out_channels,
        sample_rate: cpal::SampleRate(out_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    // Playback ring in DEVICE units (out_rate × out_channels), frame-aligned
    // so partial pushes can never split a frame. Capacity from the profile;
    // the standing occupancy the drift controller maintains is the profile's
    // latency FLOOR, not the capacity.
    let ch = out_channels.max(1) as usize;
    let lp = super::latency::params(&latency_profile, false);
    let ring_samples =
        ((out_rate as usize) * ch * (lp.adaptive_max_ms.max(lp.ring_ms) as usize) / 1000) / ch * ch;
    let rb = HeapRb::<f32>::new(ring_samples.max(ch * 512));
    let (prod, cons) = rb.split();

    let underruns = Arc::new(AtomicU64::new(0));
    let is_running = Arc::new(AtomicBool::new(true));

    // Receive thread.
    let socket = sub.socket;
    let running_net = is_running.clone();
    // Declare a missing packet lost after a handful of later packets arrive
    // (each packet is one chunk, ~5-25ms); waiting longer just plays silence.
    let lost_after = (lp.adaptive_min_ms / 20).clamp(3, 25) as usize;
    let target_samples = (out_rate as usize) * ch * (lp.adaptive_min_ms as usize) / 1000 / ch * ch;
    let pipeline = SinkPipeline::new(
        info.sample_rate.max(1),
        info.channels.max(1),
        out_rate,
        out_channels,
    );
    thread::spawn(move || {
        super::super::transport::udp::sink::receive_loop(
            &socket,
            salt_b,
            lost_after,
            key,
            nonce_salt,
            target_samples,
            out_channels,
            pipeline,
            prod,
            running_net,
        );
    });

    let stream = build_output_stream(&device, &config, out_format, cons, underruns.clone())
        .map_err(|e| e.to_string())?;

    Ok((
        stream,
        StreamStats {
            bytes_sent: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
            is_running,
            overruns: Arc::new(AtomicU64::new(0)),
            underruns,
        },
    ))
}
