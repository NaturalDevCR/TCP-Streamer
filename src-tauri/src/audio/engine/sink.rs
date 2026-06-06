//! Sink orchestrator: subscribe to a native source, open a cpal output stream
//! for the negotiated format, and pump received audio into it.

use super::playback::build_output_stream;
use super::super::stats::{emit_log, StreamStats};
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
    app_handle: AppHandle,
) -> Result<(cpal::Stream, StreamStats), String> {
    // Subscribe (blocking, off the audio thread).
    let salt_b: u64 = Instant::now().elapsed().as_nanos() as u64 ^ 0x9E37_79B9_7F4A_7C15;
    let sub = super::super::transport::udp::sink::subscribe(
        &source_addr, salt_b, Duration::from_secs(2),
    )
    .map_err(|e| format!("subscribe failed: {e}"))?;
    let info = sub.info;
    emit_log(&app_handle, "success", format!(
        "Subscribed to {} ({}Hz, {}ch)", source_addr, info.sample_rate, info.channels
    ));

    // Find the chosen output device.
    let host = cpal::default_host();
    let device = host
        .output_devices()
        .map_err(|e| e.to_string())?
        .find(|d| d.name().map(|n| n == output_device_name).unwrap_or(false))
        .ok_or_else(|| format!("Output device not found: {output_device_name}"))?;

    let config = cpal::StreamConfig {
        channels: info.channels,
        sample_rate: cpal::SampleRate(info.sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    // Playback ring sized from the latency profile.
    let lp = super::latency::params(&latency_profile, false);
    let ring_samples = (info.sample_rate as usize) * (info.channels as usize)
        * (lp.adaptive_max_ms.max(lp.ring_ms) as usize) / 1000;
    let rb = HeapRb::<f32>::new(ring_samples.max(1024));
    let (prod, cons) = rb.split();

    let underruns = Arc::new(AtomicU64::new(0));
    let is_running = Arc::new(AtomicBool::new(true));

    // Receive thread.
    let socket = sub.socket;
    let running_net = is_running.clone();
    let lost_after = (lp.ring_ms / 5).max(2) as usize; // ~frames tolerance
    thread::spawn(move || {
        super::super::transport::udp::sink::receive_loop(
            &socket, salt_b, lost_after, prod, running_net,
        );
    });

    let stream = build_output_stream(&device, &config, cons, underruns.clone())
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
