//! Sink orchestrator: subscribe to a native source, open a cpal output stream
//! in a format the device actually supports, and pump received audio into it
//! through the conversion pipeline (channel mix + resample).

use super::super::stats::{emit_log, QualityEvent, ReceiveMetrics, StatsEvent, StreamStats};
use super::convert::{SinkPipeline, MAX_TRIM_PPM};
use super::device::{negotiate, ConfigCandidate, SampleFmt};
use super::playback::build_output_stream;
use cpal::traits::DeviceTrait;
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tauri::Emitter;

#[allow(clippy::too_many_arguments)]
pub fn run_sink(
    output_device_name: String,
    source_addr: String,
    latency_profile: String,
    overrides: super::latency::LatencyOverrides,
    psk: String,
    app_handle: AppHandle,
    session_id: u64,
    is_running: Arc<AtomicBool>,
) -> Result<(cpal::Stream, StreamStats), String> {
    // Subscribe (blocking, off the audio thread). `salt_b` must be unique per
    // subscription: it feeds AEAD key/nonce derivation, and reuse across
    // sessions with the same PSK would reuse nonces. The operating system supplies
    // cryptographic randomness (the salt travels in plaintext).
    let salt_b = super::super::transport::udp::crypto::random_salt().map_err(|e| e.to_string())?;
    let mut sub = super::super::transport::udp::sink::subscribe_cancellable(
        &source_addr,
        salt_b,
        &psk,
        Duration::from_secs(2),
        &is_running,
    )
    .map_err(|e| format!("subscribe failed: {e}"))?;
    let info = sub.info;
    emit_log(
        &app_handle,
        "success",
        format!(
            "Subscribed to {} ({}Hz, {}ch)",
            source_addr, info.sample_rate, info.channels
        ),
    );

    // Find the chosen output device.
    let device = super::device::find_device(&output_device_name, true)?;

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
    // `false` for is_loopback: loopback is a capture-side concern and the sink
    // never captures. Overrides come from the user's Custom or Broadcast settings.
    let lp = super::latency::resolve(&latency_profile, false, overrides);
    super::super::session::SESSION.configure(
        session_id,
        super::super::session::EffectiveConfig {
            role: "sink".into(),
            transport: "udp".into(),
            profile: latency_profile.clone(),
            sample_rate: info.sample_rate,
            channels: info.channels,
            device_sample_rate: out_rate,
            device_channels: out_channels,
            ring_ms: lp.ring_ms.max(lp.adaptive_max_ms),
            target_ms: lp.adaptive_min_ms,
            chunk_frames: lp.chunk_size,
            capture_buffer_frames: None,
            adaptive: false,
        },
    );
    let ring_samples =
        ((out_rate as usize) * ch * (lp.adaptive_max_ms.max(lp.ring_ms) as usize) / 1000) / ch * ch;
    let rb = HeapRb::<f32>::new(ring_samples.max(ch * 512));
    let (prod, cons) = rb.split();

    let underruns = Arc::new(AtomicU64::new(0));

    let metrics = Arc::new(ReceiveMetrics::default());
    let metrics_net = metrics.clone();
    // Receive thread.
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
    let trim_ppm = Arc::new(AtomicI32::new(0));
    let trim_ppm_net = trim_ppm.clone();
    let stream = match build_output_stream(
        &device,
        &config,
        out_format,
        cons,
        underruns.clone(),
        is_running.clone(),
    ) {
        Ok(stream) => stream,
        Err(e) => {
            is_running.store(false, Ordering::Relaxed);
            return Err(e.to_string());
        }
    };

    let socket = sub.socket.try_clone().map_err(|e| e.to_string())?;
    let receiver_handle = thread::Builder::new()
        .name("udp-receiver".into())
        .spawn(move || {
            super::super::transport::udp::sink::receive_loop(
                &socket,
                salt_b,
                lost_after,
                psk,
                info,
                target_samples,
                out_rate,
                out_channels,
                pipeline,
                prod,
                running_net,
                trim_ppm_net,
                metrics_net,
            );
        })
        .map_err(|e| format!("Receiver worker could not start: {e}"))?;
    sub.disarm();

    // A separate telemetry worker samples receiver metrics and reports drift.
    // A value settling near zero means the two
    // machines' clocks are well matched, and one pinned at the cap means the
    // loop has railed and something else is wrong.
    //
    // Under a push/pop pattern close to exactly periodic, sampling the trim
    // at a fixed 10 Hz step aliases a real, sustained oscillation into the
    // reading — roughly 100 ppm peak-to-peak on a ~40 s period in the case
    // that motivated this. The control loop itself is fine (occupancy and
    // therefore standing latency hold; see the EMA at the occupancy input in
    // trim.rs and BROADCAST_GUIDE.md's Clock drift section) — this is purely
    // a reporting artifact created by sampling, and no controller filter
    // constant removes it. So the fix lives here, not in the loop: smooth
    // the *sampled* trim before deciding whether to log, and size the
    // reporting threshold for a signal that legitimately wanders by tens of
    // ppm rather than one that settles on a single number.
    let trim_ppm_log = trim_ppm.clone();
    let underruns_log = underruns.clone();
    let running_log = is_running.clone();
    let app_log = app_handle.clone();
    let telemetry_handle = thread::Builder::new()
        .name("sink-telemetry".into())
        .spawn(move || {
            let start = Instant::now();
            let mut previous_bytes = 0;
            let mut previous_report = Instant::now();
            let mut previous_underruns = 0;
            let mut previous_lost = 0;
            let mut had_audio = false;
            let mut last_packet_count = 0;
            let mut last_packet_at = Instant::now();
            // alpha = 0.02 on the 1 s samples gives a ~50 s EMA time constant:
            // long enough to knock the ~40 s aliased oscillation down by about
            // 7x in amplitude (simulated against the reviewer's push/pop case),
            // short enough to still track a genuine drift trend within the ~2
            // minutes the guide already documents for the loop to settle. This
            // EMA only decides when to log — it is not fed back into the trim.
            const REPORT_EMA_ALPHA: f64 = 0.02;
            // ppm of change in the smoothed value since the last routine report.
            const REPORT_THRESHOLD_PPM: f64 = 10.0;
            // Floor on how often routine (non-clamp) reports can fire.
            const MIN_REPORT_INTERVAL: Duration = Duration::from_secs(30);

            let mut smoothed: Option<f64> = None;
            let mut last_reported = 0.0f64;
            let mut last_report_at: Option<Instant> = None;
            let mut was_clamped = false;
            'outer: while running_log.load(Ordering::Relaxed) {
                // Wait in slices rather than one 1s block, so shutdown is noticed
                // about as fast as the receive thread notices it.
                for _ in 0..10 {
                    thread::sleep(Duration::from_millis(100));
                    if !running_log.load(Ordering::Relaxed) {
                        break 'outer;
                    }
                }
                let bytes = metrics.bytes_received.load(Ordering::Relaxed);
                let packets = metrics.received_packets.load(Ordering::Relaxed);
                if packets != last_packet_count {
                    last_packet_count = packets;
                    last_packet_at = Instant::now();
                }
                let audio_active = packets > 0 && last_packet_at.elapsed() < Duration::from_secs(3);
                if audio_active != had_audio {
                    had_audio = audio_active;
                    super::super::session::publish(
                        &app_log,
                        session_id,
                        if audio_active {
                            "running"
                        } else {
                            "reconnecting"
                        },
                        if audio_active {
                            "Receiving audio"
                        } else {
                            "Waiting for source audio"
                        },
                    );
                }
                let wire = metrics.wire_format.load(Ordering::Relaxed);
                if let Some(status) = super::super::session::SESSION.update_runtime(
                    session_id,
                    (wire >> 16) as u32,
                    wire as u16,
                    lp.adaptive_min_ms,
                ) {
                    let _ = app_log.emit("stream-state", status);
                }
                let occupied = metrics.buffered_samples.load(Ordering::Relaxed);
                let lost = metrics.lost_frames.load(Ordering::Relaxed);
                let underrun_samples = underruns_log.load(Ordering::Relaxed);
                let glitches = u64::from(underrun_samples > previous_underruns)
                    + u64::from(lost > previous_lost);
                previous_underruns = underrun_samples;
                previous_lost = lost;
                let _ = app_log.emit(
                    "stats-event",
                    StatsEvent {
                        session_id,
                        received: true,
                        uptime_seconds: start.elapsed().as_secs(),
                        bytes_sent: bytes,
                        bitrate_kbps: bytes.saturating_sub(previous_bytes) as f64 * 8.0
                            / (previous_report.elapsed().as_secs_f64() * 1000.0),
                        buffer_ms: occupied as f32 * 1000.0
                            / (out_rate as f32 * out_channels as f32),
                        target_ms: lp.adaptive_min_ms,
                        sample_rate: out_rate,
                        channels: out_channels,
                        auth_failures: metrics.auth_failures.load(Ordering::Relaxed),
                        lost_frames: lost,
                        trim_ppm: trim_ppm_log.load(Ordering::Relaxed),
                    },
                );
                previous_bytes = bytes;
                previous_report = Instant::now();
                let occupancy = occupied as f32 / ring_samples.max(1) as f32;
                let _ = app_log.emit(
                    "quality-event",
                    QualityEvent {
                        session_id,
                        score: if audio_active {
                            super::super::metrics::quality_score(glitches, occupancy, None)
                        } else {
                            0
                        },
                        rtt_ms: None,
                        rtt_var_ms: None,
                        underruns: underrun_samples / u64::from(out_channels.max(1)),
                        dropped: lost,
                        counter_unit: "frames/frames".into(),
                        buffer_health: occupancy,
                    },
                );
                let ppm = trim_ppm_log.load(Ordering::Relaxed);

                // The clamp is the "something else is wrong" signal, and it must
                // reach the operator promptly regardless of smoothing or the
                // routine rate limit: check the raw sample, not the EMA, and
                // report once per entry into the railed state rather than once
                // per sample while it persists.
                let is_clamped = (ppm as f64).abs() >= MAX_TRIM_PPM;
                if is_clamped {
                    if !was_clamped {
                        emit_log(
                            &app_log,
                            "warning",
                            format!(
                            "Clock drift correction: {ppm} ppm (at limit — check clocks/network)"
                        ),
                        );
                        last_reported = ppm as f64;
                        last_report_at = Some(Instant::now());
                    }
                    was_clamped = true;
                    smoothed = Some(ppm as f64);
                    continue;
                }
                was_clamped = false;

                let s = match smoothed {
                    Some(prev) => REPORT_EMA_ALPHA * (ppm as f64) + (1.0 - REPORT_EMA_ALPHA) * prev,
                    None => ppm as f64,
                };
                smoothed = Some(s);

                let interval_elapsed = last_report_at
                    .map(|t| t.elapsed() >= MIN_REPORT_INTERVAL)
                    .unwrap_or(true);
                if (s - last_reported).abs() > REPORT_THRESHOLD_PPM && interval_elapsed {
                    emit_log(
                        &app_log,
                        "info",
                        format!("Clock drift correction: {:.0} ppm", s),
                    );
                    last_reported = s;
                    last_report_at = Some(Instant::now());
                }
            }
        });
    let telemetry_handle = match telemetry_handle {
        Ok(handle) => handle,
        Err(e) => {
            is_running.store(false, Ordering::Relaxed);
            let _ = receiver_handle.join();
            return Err(format!("Telemetry worker could not start: {e}"));
        }
    };

    Ok((
        stream,
        StreamStats {
            workers: vec![receiver_handle, telemetry_handle],
            bytes_sent: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
            is_running,
            overruns: Arc::new(AtomicU64::new(0)),
            underruns,
        },
    ))
}
