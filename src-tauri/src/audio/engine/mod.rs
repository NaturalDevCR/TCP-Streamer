//! Audio engine: device selection, capture, and (later) buffering.

pub mod buffer;
pub mod capture;
pub mod convert;
pub mod decoder;
pub mod device;
pub mod encoder;
pub mod latency;
pub mod pacing;
pub mod playback;
pub mod sink;

/// The wire format is always interleaved stereo (s16le at the configured
/// output rate); the capture side normalizes channels in the callback and the
/// send loop resamples, so the ring always holds stereo at the capture rate.
const STEREO: u16 = 2;

// ── Imports for the engine::run orchestrator ──
use super::chunked::ChunkedWriter;
use super::constants::*;
use super::stats::{emit_log, BufferResizeEvent, QualityEvent, StatsEvent, StreamStats};
use super::wav_helper::create_wav_header;
use cpal::traits::{DeviceTrait, HostTrait};

use ringbuf::HeapRb;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use thread_priority::{ThreadBuilder, ThreadPriority};

/// Binds a listener that accepts both IPv6 and IPv4 (via an IPv6 dual-stack
/// socket). Falls back to IPv4-only if the dual-stack bind fails (e.g. IPv6
/// disabled). Returns the listener and a label describing what it bound to.
fn bind_dual_stack(port: u16) -> std::io::Result<(std::net::TcpListener, &'static str)> {
    use socket2::{Domain, Protocol, Socket, Type};
    use std::net::{Ipv6Addr, SocketAddr};

    let try_v6 = || -> std::io::Result<std::net::TcpListener> {
        let socket = Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP))?;
        socket.set_only_v6(false)?;
        let _ = socket.set_reuse_address(true);
        let addr: SocketAddr = (Ipv6Addr::UNSPECIFIED, port).into();
        socket.bind(&addr.into())?;
        socket.listen(128)?;
        Ok(socket.into())
    };

    match try_v6() {
        Ok(l) => Ok((l, "[::] (dual-stack)")),
        Err(_) => {
            let l = std::net::TcpListener::bind(("0.0.0.0", port))?;
            Ok((l, "0.0.0.0 (IPv4 only)"))
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    device_name: String,
    ip: String,
    port: u16,
    sample_rate: u32,
    buffer_size: u32,
    ring_buffer_duration_ms: u32,
    high_priority: bool,
    dscp_strategy: String,
    format: String,
    chunk_size: u32,
    is_loopback: bool,
    is_server: bool,
    auto_reconnect: bool,
    enable_adaptive_buffer: bool,
    min_buffer_ms: u32,
    max_buffer_ms: u32,
    latency_profile: String,
    allowlist: String,
    transport: String,
    psk: String,
    app_handle: AppHandle,
) -> Result<(cpal::Stream, StreamStats), String> {
    emit_log(
        &app_handle,
        "debug",
        format!(
            "Init stream: Mode={} Protocol={} Device='{}', Rate={}, RingMs={}, Priority={}, DSCP={}, Format={}, Chunk={}, Loopback={}, AdaptiveBuf={} ({}ms-{}ms)",
            if is_server { "SERVER" } else { "CLIENT" }, transport, device_name, sample_rate, ring_buffer_duration_ms, high_priority, dscp_strategy, format, chunk_size, is_loopback, enable_adaptive_buffer, min_buffer_ms, max_buffer_ms
        ),
    );

    let host = cpal::default_host();

    // Device selection logic
    let device = if is_loopback {
        // WASAPI loopback exists only on Windows; elsewhere an input stream on
        // an output device "builds" but never delivers a single sample.
        if std::env::consts::OS != "windows" {
            return Err(
                "Loopback (system audio) capture is only supported on Windows. \
                 Select a real input device."
                    .to_string(),
            );
        }
        // Loopback mode: Search in OUTPUT devices
        let clean_name = device_name.replace("[Loopback] ", "");
        let mut found_device = None;

        let available_devices = host.output_devices().map_err(|e| e.to_string())?;
        let mut device_list_log = Vec::new();

        for dev in available_devices {
            if let Ok(name) = dev.name() {
                device_list_log.push(name.clone());
                if name == clean_name || name.trim() == clean_name.trim() {
                    found_device = Some(dev);
                    break;
                }
            }
        }

        if found_device.is_none() {
            emit_log(
                &app_handle,
                "error",
                format!(
                    "Loopback device '{}' not found. Available outputs: {:?}",
                    clean_name, device_list_log
                ),
            );
        }

        found_device.ok_or_else(|| format!("Loopback device not found: {}", clean_name))?
    } else {
        // Standard mode: Search in INPUT devices
        let mut found_device = None;
        if let Ok(devices) = host.input_devices() {
            for dev in devices {
                if let Ok(name) = dev.name() {
                    if name == device_name {
                        found_device = Some(dev);
                        break;
                    }
                }
            }
        }
        found_device.ok_or_else(|| format!("Input device not found: {}", device_name))?
    };

    // 0.5. Format contract: `sample_rate` is the OUTPUT (wire) rate — what the
    // receiver (e.g. Snapserver's `48000:16:2` source) expects. The device is
    // opened at a rate IT actually supports (capture_rate, negotiated below)
    // and at its native channel count; the capture callback mixes to stereo
    // and the send loop resamples capture_rate → wire rate before encoding.
    // We never rely on the OS to resample for us: that assumption only held
    // on some backends (WASAPI shared mode, PulseAudio) and produced
    // pitch-shifted "chipmunk" audio everywhere else.

    // Detect supported audio formats from device
    let supported_configs: Vec<_> = if is_loopback {
        device
            .supported_output_configs()
            .map_err(|e| e.to_string())?
            .collect()
    } else {
        device
            .supported_input_configs()
            .map_err(|e| e.to_string())?
            .collect()
    };

    use self::device::{negotiate, ConfigCandidate, SampleFmt};

    let to_fmt = |f: cpal::SampleFormat| match f {
        cpal::SampleFormat::F32 => Some(SampleFmt::F32),
        cpal::SampleFormat::I16 => Some(SampleFmt::I16),
        cpal::SampleFormat::U16 => Some(SampleFmt::U16),
        cpal::SampleFormat::I32 => Some(SampleFmt::I32),
        _ => None,
    };

    // Pair each usable cpal config with its simplified candidate view so the
    // negotiated index maps straight back to the cpal range.
    let usable: Vec<(ConfigCandidate, &cpal::SupportedStreamConfigRange)> = supported_configs
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

    let negotiated = negotiate(&candidates, sample_rate).ok_or_else(|| {
        format!(
            "No usable capture config: device offers none of the supported \
             sample formats (f32/i16/u16/i32). Available: {:?}",
            supported_configs
                .iter()
                .map(|r| r.sample_format())
                .collect::<Vec<_>>()
        )
    })?;
    let config_range = *usable[negotiated.index].1;
    let selected_format = config_range.sample_format();
    let device_channels = config_range.channels();

    // Wire format is FIXED: `sample_rate`:16:2 (s16le). Capture runs at
    // whatever the device supports; the engine converts in between.
    let wire_rate = sample_rate;
    let capture_rate = negotiated.capture_rate;

    if capture_rate != wire_rate {
        emit_log(
            &app_handle,
            "info",
            format!(
                "Device does not support {}Hz natively; capturing at {}Hz \
                 and resampling internally.",
                wire_rate, capture_rate
            ),
        );
    }
    emit_log(
        &app_handle,
        "info",
        format!(
            "Audio Input: format={:?}, {}ch @ {}Hz → wire: s16le 2ch @ {}Hz",
            selected_format, device_channels, capture_rate, wire_rate
        ),
    );

    let stream_config = cpal::StreamConfig {
        channels: device_channels,
        sample_rate: cpal::SampleRate(capture_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    emit_log(
        &app_handle,
        "info",
        format!(
            "Capture buffer: Default (OS driver chooses optimal size, requested {} frames)",
            buffer_size
        ),
    );

    // 1. Setup Ring Buffer (latency profile drives the sizes; "custom" uses the
    // user's manual fields). The ring is CAPACITY (stall absorption); the
    // adaptive band is the standing-latency target enforced by the send loop.
    let lp = if latency_profile == "custom" {
        self::latency::LatencyParams {
            ring_ms: ring_buffer_duration_ms,
            adaptive_min_ms: min_buffer_ms,
            adaptive_max_ms: max_buffer_ms,
            chunk_size,
            prefill_ms: ring_buffer_duration_ms.min(200),
        }
    } else {
        self::latency::params(&latency_profile, is_loopback)
    };
    let effective_chunk = lp.chunk_size;

    let ring_capacity_ms = lp.adaptive_max_ms.max(lp.ring_ms);
    // Ring holds interleaved STEREO at the capture rate. Capacity must stay
    // even so a partial push/pop can never split a stereo frame.
    let ring_buffer_size =
        ((capture_rate as usize) * (STEREO as usize) * (ring_capacity_ms as usize) / 1000) & !1;

    let buffer_size_mb = (ring_buffer_size * std::mem::size_of::<f32>()) as f32 / (1024.0 * 1024.0);

    emit_log(
        &app_handle,
        "info",
        format!(
            "Profile '{}': ring capacity {}ms ({:.2}MB), latency target {}-{}ms, chunk {}, prefill {}ms - Device type: {}",
            latency_profile,
            ring_capacity_ms,
            buffer_size_mb,
            lp.adaptive_min_ms,
            lp.adaptive_max_ms,
            lp.chunk_size,
            lp.prefill_ms,
            if is_loopback {
                "WASAPI Loopback"
            } else {
                "Standard Input"
            }
        ),
    );

    let rb = HeapRb::<f32>::new(ring_buffer_size);
    let (prod, mut cons) = rb.split();

    // Shared stats
    let bytes_sent = Arc::new(AtomicU64::new(0));
    let is_running = Arc::new(AtomicBool::new(true));
    let overruns = Arc::new(AtomicU64::new(0));
    let underruns = Arc::new(AtomicU64::new(0));

    let bytes_sent_clone = bytes_sent.clone();
    let is_running_clone = is_running.clone();
    let app_handle_net = app_handle.clone();
    let ip_clone = ip.clone();
    let format_clone = format.clone();
    let wire_rate_net = wire_rate;
    let capture_rate_net = capture_rate;
    let is_server_clone = is_server;
    let auto_reconnect_net = auto_reconnect;
    let dscp_clone = dscp_strategy.clone();
    let overruns_net = overruns.clone();
    let underruns_net = underruns.clone();
    let effective_chunk_net = effective_chunk;
    let transport_net = transport.clone();
    let psk_net = psk.clone();

    let allow_rules = super::transport::allowlist::parse_rules(&allowlist);

    // 2. Spawn Network Thread (Consumer)
    let priority = if high_priority {
        ThreadPriority::Max
    } else {
        ThreadPriority::Min
    };
    let thread_builder = ThreadBuilder::default()
        .name("NetworkThread")
        .priority(priority);

    let _ = thread_builder.spawn(move |result| {
        if let Err(e) = result {
            emit_log(
                &app_handle_net,
                "warning",
                format!("Failed to set thread priority: {:?}", e),
            );
        } else if high_priority {
            emit_log(
                &app_handle_net,
                "info",
                "Network thread priority set to Max".to_string(),
            );
        }

        // Bind listener if in server mode
        let listener = if is_server_clone {
            match bind_dual_stack(port) {
                Ok((l, label)) => {
                    let _ = l.set_nonblocking(true);
                    emit_log(&app_handle_net, "success", format!("TCP Server listening on {} (port {})", label, port));
                    Some(l)
                }
                Err(e) => {
                    emit_log(&app_handle_net, "error", format!("Failed to bind TCP server port: {}", e));
                    None
                }
            }
        } else {
            None
        };


        let mut temp_buffer = vec![0.0f32; effective_chunk_net as usize * STEREO as usize];
        let start_time = Instant::now();
        let mut last_stats_emit = Instant::now();

        // Quality tracking
        let mut last_quality_emit = Instant::now();

        let adaptive_min_ms = lp.adaptive_min_ms;

        // Start at the floor: lowest standing latency; real glitches raise it.
        let mut adaptive = self::buffer::AdaptiveBuffer::new(
            adaptive_min_ms,
            lp.adaptive_max_ms.max(adaptive_min_ms),
            super::constants::ADAPTIVE_BUFFER_STEP_MS,
            adaptive_min_ms,
            6,
        );
        let mut glitch_baseline: u64 = 0;
        let mut starvation = self::pacing::StarvationGate::new(STARVATION_THRESHOLD_MS);

        let mut retry_delay = Duration::from_secs(INITIAL_RETRY_DELAY_SECS);

        fn add_jitter(base: Duration) -> Duration {
            use std::time::SystemTime;
            let jitter_ms = (SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
                % 1000) as i64
                - 500;
            let ms = base.as_millis() as i64 + jitter_ms;
            Duration::from_millis(ms.max(MIN_RETRY_DELAY_MS as i64) as u64)
        }

        let mut current_stream: Option<Box<dyn super::transport::Connection>> = None;
        let mut disconnect_time: Option<Instant> = None;

        emit_log(
            &app_handle_net,
            "info",
            "Network thread started".to_string(),
        );

        let mut last_heartbeat = Instant::now();

        let mut prefilled = false;
        let prefill_ms = lp.prefill_ms;
        let prefill_samples =
            capture_rate_net as usize * STEREO as usize * prefill_ms as usize / 1000;
        emit_log(
            &app_handle_net,
            "info",
            format!(
                "Buffering... waiting for {} samples ({}ms)",
                prefill_samples, prefill_ms
            ),
        );

        let mut use_chunked = false;
        let mut wav_header_sent = false;
        let mut prefill_debug_timer = Instant::now();
        let mut payload: Vec<u8> = Vec::new();
        // capture_rate → wire_rate conversion happens here, on the network
        // thread, right before encoding (passthrough when rates match).
        let mut resampler = self::convert::StereoResampler::new(capture_rate_net, wire_rate_net);
        let mut resampled: Vec<f32> = Vec::new();
        let mut udp_source: Option<super::transport::udp::source::UdpSource> = None;
        let mut advertiser: Option<super::transport::discovery::Advertiser> = None;

        while is_running_clone.load(Ordering::Relaxed) {
            // 1. Hardware-Driven Pacing
            let current_buffered = cons.len();
            let min_chunk_samples = effective_chunk_net as usize * STEREO as usize;

            // Honest underrun detection: an instantaneously empty ring is the
            // normal steady state (we drain faster than capture fills); only
            // SUSTAINED emptiness while connected counts as a real glitch.
            let starved = current_stream.is_some() && prefilled && current_buffered < min_chunk_samples;
            if starvation.update(starved, start_time.elapsed().as_millis() as u64) {
                underruns_net.fetch_add(1, Ordering::Relaxed);
                emit_log(
                    &app_handle_net,
                    "warning",
                    "Capture starvation: ring stayed empty while connected (device stalled?)".to_string(),
                );
            }

            if transport_net != "udp" && current_stream.is_none() && current_buffered > 0 {
                // Track how long we've been disconnected
                if disconnect_time.is_none() {
                    disconnect_time = Some(Instant::now());
                }

                let disconnected_for = disconnect_time.expect("disconnect_time set above").elapsed();

                if disconnected_for >= DISCONNECT_FLUSH_THRESHOLD {
                    // Prolonged outage: flush entire buffer to avoid sending stale audio on reconnect
                    let flushed = cons.len();
                    let mut drain = vec![0.0f32; flushed];
                    let _ = cons.pop_slice(&mut drain);
                    prefilled = false;
                    emit_log(
                        &app_handle_net,
                        "warning",
                        format!(
                            "Buffer flushed ({} samples) after {:.1}s disconnect. Waiting for fresh audio.",
                            flushed,
                            disconnected_for.as_secs_f32()
                        ),
                    );
                } else {
                    // Short outage: drain gradually (whole stereo frames only)
                    let drain_amount = ((capture_rate_net as usize * STEREO as usize / 10)
                        .min(current_buffered))
                        & !1;
                    let mut drain_buffer = vec![0.0f32; drain_amount];
                    let _ = cons.pop_slice(&mut drain_buffer);
                }
                thread::sleep(Duration::from_millis(10));
            } else if starved {
                // Hardware-driven synchronization: sleep until we have enough samples for a FULL chunk
                thread::sleep(Duration::from_millis(2));
                continue;
            }

            // Periodic heartbeat
            if last_heartbeat.elapsed() >= Duration::from_secs(HEARTBEAT_INTERVAL_SECS) {
                let occupied = cons.len();
                let capacity = cons.capacity();
                let buffer_pct = occupied as f32 / capacity as f32 * 100.0;
                emit_log(
                    &app_handle_net,
                    "debug",
                    format!(
                        "Network thread heartbeat: ✓ Active | Buffer: {:.1}% | Connection: {}",
                        buffer_pct,
                        if current_stream.is_some() {
                            "Connected"
                        } else {
                            "Disconnected"
                        }
                    ),
                );
                last_heartbeat = Instant::now();
            }

            // Native UDP transport (Phase 2B)
            if transport_net == "udp" {
                // Lazily bind the UDP source on first iteration.
                if udp_source.is_none() {
                    match super::transport::udp::source::UdpSource::bind(port, wire_rate_net, STEREO, psk_net.clone()) {
                        Ok(s) => { emit_log(&app_handle_net, "success", format!("Native UDP source on port {}", port)); udp_source = Some(s); }
                        Err(e) => { emit_log(&app_handle_net, "error", format!("UDP bind failed: {}", e)); thread::sleep(Duration::from_millis(500)); }
                    }
                }
                if advertiser.is_none() {
                    let name = format!("tcp-streamer-{}", port);
                    match super::transport::discovery::advertise(&name, port, !psk_net.is_empty()) {
                        Ok(a) => { advertiser = Some(a); emit_log(&app_handle_net, "info", "Advertised via mDNS".to_string()); }
                        Err(e) => emit_log(&app_handle_net, "warning", format!("mDNS advertise failed: {}", e)),
                    }
                }
                if let Some(src) = udp_source.as_mut() {
                    src.poll_subscribe();
                    if src.has_peer() {
                        // Standing-latency enforcement (same policy as TCP):
                        // after a stall, drop backlog above the adaptive target
                        // instead of letting it become permanent latency.
                        let excess = self::pacing::excess_samples(
                            cons.len(),
                            adaptive.target_ms(),
                            DRAIN_MARGIN_MS,
                            capture_rate_net,
                            STEREO,
                        );
                        if excess > 0 {
                            let skipped = cons.skip(excess);
                            overruns_net.fetch_add(skipped as u64, Ordering::Relaxed);
                            let dropped_ms = skipped * 1000
                                / (capture_rate_net as usize * STEREO as usize);
                            emit_log(
                                &app_handle_net,
                                "warning",
                                format!(
                                    "Catch-up: dropped {}ms of backlog to hold latency near {}ms",
                                    dropped_ms,
                                    adaptive.target_ms()
                                ),
                            );
                        }
                        let count = cons.pop_slice(&mut temp_buffer);
                        if count > 0 {
                            resampler.process(&temp_buffer[..count], &mut resampled);
                            self::encoder::encode_f32_to_pcm_i16_le(&resampled, &mut payload);
                            src.send_audio(&payload);
                            let _ = bytes_sent_clone.fetch_add(payload.len() as u64, Ordering::Relaxed);
                        } else {
                            thread::sleep(Duration::from_millis(1));
                        }
                    } else {
                        // No subscriber: keep the ring fresh so a new peer
                        // starts live instead of receiving stale backlog, and
                        // capture-side overruns don't pile up while idle.
                        let stale = cons.len();
                        if stale > 0 {
                            let _ = cons.skip(stale);
                        }
                        thread::sleep(Duration::from_millis(20));
                    }
                }
            }

            // Stats Logic
            if last_stats_emit.elapsed() >= Duration::from_secs(2) {
                let uptime = start_time.elapsed().as_secs();
                let current_bytes = bytes_sent_clone.load(Ordering::Relaxed);
                let bitrate = if uptime > 0 {
                    (current_bytes as f64 * 8.0) / (uptime as f64 * 1000.0)
                } else {
                    0.0
                };
                let _ = app_handle_net.emit(
                    "stats-event",
                    StatsEvent {
                        uptime_seconds: uptime,
                        bytes_sent: current_bytes,
                        bitrate_kbps: bitrate,
                    },
                );
                last_stats_emit = Instant::now();
            }

            // Quality Logic (honest metrics)
            if last_quality_emit.elapsed() >= Duration::from_secs(QUALITY_REPORT_INTERVAL_SECS) {
                let occupied = cons.len();
                let capacity = cons.capacity();
                let buffer_health = 1.0 - (occupied as f32 / capacity as f32);
                let occupancy_ratio = occupied as f32 / capacity as f32;

                let total_glitches =
                    underruns_net.load(Ordering::Relaxed) + overruns_net.load(Ordering::Relaxed);
                let glitches_delta = total_glitches.saturating_sub(glitch_baseline);
                glitch_baseline = total_glitches;

                // Best-effort RTT from the live socket, if connected.
                let rtt = current_stream.as_ref().and_then(|s| s.rtt());

                let score = super::metrics::quality_score(
                    glitches_delta,
                    occupancy_ratio,
                    rtt.map(|r| r.srtt_ms),
                );

                let _ = app_handle_net.emit(
                    "quality-event",
                    QualityEvent {
                        score,
                        rtt_ms: rtt.map(|r| r.srtt_ms),
                        rtt_var_ms: rtt.map(|r| r.rttvar_ms),
                        underruns: underruns_net.load(Ordering::Relaxed),
                        dropped: overruns_net.load(Ordering::Relaxed),
                        buffer_health,
                    },
                );
                last_quality_emit = Instant::now();

                // Real adaptive control: one tick per quality interval.
                if enable_adaptive_buffer {
                    if let Some(new_target) = adaptive.on_tick(glitches_delta > 0) {
                        emit_log(
                            &app_handle_net,
                            "info",
                            format!("Adaptive Buffer: target now {}ms", new_target),
                        );
                        let _ = app_handle_net.emit(
                            "buffer-resize",
                            BufferResizeEvent {
                                new_size_ms: new_target,
                                reason: if glitches_delta > 0 {
                                    "Glitches detected"
                                } else {
                                    "Stable"
                                }
                                .to_string(),
                            },
                        );
                    }
                }
            }

            if transport_net == "udp" {
                continue;
            }

            // 2. Connection Management
            if current_stream.is_none() {
                if is_server_clone {
                    // SERVER MODE
                    if let Some(l) = &listener {
                        // TCP Server
                        match l.accept() {
                            Ok((mut stream, addr)) => {
                                if !super::transport::allowlist::is_allowed(addr.ip(), &allow_rules, true) {
                                    emit_log(&app_handle_net, "warning", format!("Rejected connection from {} (not in allowlist)", addr.ip()));
                                    let _ = stream.shutdown(std::net::Shutdown::Both);
                                    continue;
                                }
                                let _ = stream.set_nodelay(true);

                                // Apply DSCP/QoS to the accepted client connection (parity with client mode).
                                let tos_val = super::transport::dscp::dscp_to_tos(&dscp_clone);
                                if tos_val > 0 {
                                    let sref = socket2::SockRef::from(&stream);
                                    if let Err(e) = sref.set_tos(u32::from(tos_val)) {
                                        emit_log(&app_handle_net, "warning", format!("Failed to set server QoS/TOS: {}", e));
                                    }
                                }

                                // Bounded, non-blocking handshake (no thread stall).
                                stream.set_nonblocking(true).ok();
                                let mut peekbuf = [0u8; 8];
                                let mut is_http = false;
                                let deadline = Instant::now() + Duration::from_millis(50);
                                loop {
                                    match stream.peek(&mut peekbuf) {
                                        Ok(n) if n >= 4 => {
                                            is_http = super::transport::tcp_server::looks_like_http(&peekbuf[..n]);
                                            break;
                                        }
                                        Ok(_) => {}
                                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                                        Err(_) => break,
                                    }
                                    if Instant::now() >= deadline {
                                        break;
                                    }
                                    thread::sleep(Duration::from_millis(2));
                                }
                                stream.set_nonblocking(false).ok();

                                if is_http {
                                    let mut hdr = [0u8; 1024];
                                    let _ = stream.read(&mut hdr); // consume request line (best-effort)
                                    let resp = super::transport::tcp_server::http_stream_headers("audio/wav");
                                    if let Err(e) = stream.write_all(resp.as_bytes()) {
                                        emit_log(&app_handle_net, "error", format!("Failed to send HTTP headers: {}", e));
                                    } else {
                                        emit_log(&app_handle_net, "info", "HTTP client detected. Serving audio/wav".to_string());
                                    }
                                }

                                let request_format = if is_http { "wav" } else { format_clone.as_str() };
                                emit_log(&app_handle_net, "success", format!("Client connected from {} (Format: {})", addr, request_format));
                                use_chunked = is_http;
                                wav_header_sent = false;
                                current_stream = Some(Box::new(super::transport::TcpConnection::new(stream)));
                                disconnect_time = None;
                                last_heartbeat = Instant::now();
                            }
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                            Err(e) => {
                                emit_log(&app_handle_net, "error", format!("Accept error: {}", e));
                            }
                        }
                    }
                } else {
                    // CLIENT MODE (TCP only)
                    match super::transport::tcp_client::connect(
                        &ip_clone, port, &dscp_clone, Duration::from_secs(2),
                    ) {
                        Ok(stream) => {
                            emit_log(&app_handle_net, "success", format!("Connected to {}:{}", ip_clone, port));
                            current_stream = Some(Box::new(super::transport::TcpConnection::new(stream)));
                            disconnect_time = None;
                            retry_delay = Duration::from_secs(2);
                            last_heartbeat = Instant::now();
                        }
                        Err(e) => {
                            emit_log(&app_handle_net, "error", format!("Connection failed: {}", e));
                            if !auto_reconnect_net {
                                emit_log(
                                    &app_handle_net,
                                    "info",
                                    "Auto-reconnect disabled; stopping stream.".to_string(),
                                );
                                is_running_clone.store(false, Ordering::Relaxed);
                                continue;
                            }
                            thread::sleep(add_jitter(retry_delay));
                            retry_delay = (retry_delay * 2).min(MAX_RETRY_DELAY);
                        }
                    }
                }
            }

            // 3. Send Data
            if let Some(mut stream_socket) = current_stream.take() {
                // Send WAV header if needed
                // Server Mode:
                // - HTTP client (use_chunked): always WAV — we advertised
                //   Content-Type: audio/wav in the response headers.
                // - "wav": Header + PCM (Browsers/VLC over raw TCP)
                // - "pcm": Raw PCM (audio receiver)
                if is_server_clone && (use_chunked || format_clone == "wav") && !wav_header_sent {
                     {
                         // Send header with 0 length (handled by helper) for unknown/stream
                         let header = create_wav_header(wire_rate_net, STEREO, 16);
                         let stream: &mut dyn super::transport::Connection = stream_socket.as_mut();

                         if use_chunked {
                             let mut writer = ChunkedWriter::new(stream);
                             if let Err(e) = writer.write_all(&header) {
                                  let level = if e.kind() == std::io::ErrorKind::BrokenPipe { "info" } else { "error" };
                                  emit_log(&app_handle_net, level, format!("Failed to send WAV header (chunked): {}", e));
                             } else {
                                 wav_header_sent = true;
                             }
                          } else if let Err(e) = stream.write_all(&header) {
                                  let level = if e.kind() == std::io::ErrorKind::BrokenPipe { "info" } else { "error" };
                                  emit_log(&app_handle_net, level, format!("Failed to send WAV header: {}", e));
                          } else {
                                   wav_header_sent = true;
                              }
                      }
                }

                // Check prefill status
                if !prefilled {
                    if cons.len() >= prefill_samples {
                        prefilled = true;
                        emit_log(
                            &app_handle_net,
                            "success",
                            "Buffer prefilled! Starting transmission.".to_string(),
                        );
                    } else {
                        // Not ready yet, put stream back and skip sending
                         if prefill_debug_timer.elapsed().as_secs() >= 5 {
                             prefill_debug_timer = Instant::now();
                             emit_log(&app_handle_net, "warning", format!("Buffering... {}/{} samples. (Ensure audio is playing)", cons.len(), prefill_samples));
                        }

                        current_stream = Some(stream_socket);
                        thread::sleep(Duration::from_millis(10)); // Prevent tight loop
                        continue;
                    }
                }

                // Standing-latency enforcement: a stall (slow socket, brief
                // outage) leaves backlog that would otherwise persist as
                // permanent end-to-end latency. Drop back to the adaptive
                // target; the drop counts as a glitch so the target can grow.
                let excess = self::pacing::excess_samples(
                    cons.len(),
                    adaptive.target_ms(),
                    DRAIN_MARGIN_MS,
                    capture_rate_net,
                    STEREO,
                );
                if excess > 0 {
                    let skipped = cons.skip(excess);
                    overruns_net.fetch_add(skipped as u64, Ordering::Relaxed);
                    let dropped_ms =
                        skipped * 1000 / (capture_rate_net as usize * STEREO as usize);
                    emit_log(
                        &app_handle_net,
                        "warning",
                        format!(
                            "Catch-up: dropped {}ms of backlog to hold latency near {}ms",
                            dropped_ms,
                            adaptive.target_ms()
                        ),
                    );
                }

                let count = cons.pop_slice(&mut temp_buffer);

                if count > 0 {
                    resampler.process(&temp_buffer[..count], &mut resampled);
                    self::encoder::encode_f32_to_pcm_i16_le(&resampled, &mut payload);

                    let _write_start = Instant::now();
                    let mut write_success = false;
                    let mut would_block = false;

                    {
                        let stream: &mut dyn super::transport::Connection = stream_socket.as_mut();
                        if use_chunked {
                            let mut writer = ChunkedWriter::new(stream);
                            match writer.write_all(&payload) {
                                 Ok(_) => write_success = true,
                                 Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => { would_block = true; },
                                 Err(e) => {
                                      emit_log(&app_handle_net, "error", format!("Write error (chunked): {}", e));
                                 }
                            }
                        } else {
                            match stream.write_all(&payload) {
                                Ok(_) => write_success = true,
                                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => { would_block = true; },
                                Err(e) => {
                                     let (level, msg) = if e.kind() == std::io::ErrorKind::BrokenPipe {
                                         ("info", "Client disconnected".to_string())
                                     } else {
                                         ("error", format!("Write error: {}", e))
                                     };
                                     emit_log(&app_handle_net, level, msg);
                                }
                            }
                        }
                    }

                    if write_success {
                        let _ = bytes_sent_clone.fetch_add(payload.len() as u64, Ordering::Relaxed);

                        current_stream = Some(stream_socket);
                    } else if would_block {
                        current_stream = Some(stream_socket);
                    } else {
                         // Write error — disconnect
                         stream_socket.close();
                         current_stream = None;

                         if !is_server_clone && !auto_reconnect_net {
                             emit_log(&app_handle_net, "info", "Auto-reconnect disabled; stopping stream.".to_string());
                             is_running_clone.store(false, Ordering::Relaxed);
                         }

                         thread::sleep(Duration::from_millis(100));
                    }
                } else {
                    current_stream = Some(stream_socket);
                }
            }
        }

        let exit_reason = if !is_running_clone.load(Ordering::Relaxed) {
            "Stopped by user"
        } else {
            "Unexpected termination"
        };
        emit_log(
            &app_handle_net,
            "info",
            format!("Network thread stopped: {}", exit_reason),
        );
    });

    // 3. Build Audio Stream (Producer) via RT-safe capture — no Arc<Mutex> in callback
    let capacity_hint = effective_chunk as usize * STEREO as usize;
    let audio_stream = self::capture::build_input_stream(
        &device,
        &stream_config,
        selected_format,
        device_channels,
        prod,
        overruns.clone(),
        capacity_hint,
    )
    .map_err(|e| e.to_string())?;

    Ok((
        audio_stream,
        StreamStats {
            bytes_sent,
            start_time: Instant::now(),
            is_running,
            overruns,
            underruns,
        },
    ))
}

/// End-to-end pipeline test: a multichannel, non-48k "device" must come out
/// the other side as stereo 48k s16le with pitch and channel polarity intact.
/// This is the regression test for the post-refactor "chipmunk audio": the
/// wire format is fixed, no matter what the capture device looks like.
#[cfg(test)]
mod pipeline_tests {
    use super::*;

    #[test]
    fn multichannel_44k1_device_produces_stereo_48k_wire() {
        const IN_RATE: u32 = 44_100;
        const WIRE_RATE: u32 = 48_000;
        const IN_CH: usize = 16;
        const SECONDS: f32 = 2.0;
        const FREQ: f32 = 440.0;

        // Ring as the engine allocates it: stereo @ capture rate, even size.
        let ring_size = ((IN_RATE as usize) * 2 * 4000 / 1000) & !1;
        let rb = HeapRb::<f32>::new(ring_size);
        let (mut prod, mut cons) = rb.split();
        let overruns = Arc::new(AtomicU64::new(0));

        let mut resampler = self::super::convert::StereoResampler::new(IN_RATE, WIRE_RATE);
        let mut scratch: Vec<f32> = Vec::new();
        let mut temp = vec![0.0f32; 512 * 2];
        let mut resampled: Vec<f32> = Vec::new();
        let mut payload: Vec<u8> = Vec::new();
        let mut wire: Vec<u8> = Vec::new();

        // Drive "callbacks" of 441 frames (10ms) of a 16ch device whose front
        // pair carries L = sine, R = -sine; other channels carry garbage that
        // must never reach the wire.
        let total_frames = (IN_RATE as f32 * SECONDS) as usize;
        let mut frame_idx = 0usize;
        let mut callback = Vec::new();
        while frame_idx < total_frames {
            let n = 441.min(total_frames - frame_idx);
            callback.clear();
            for k in 0..n {
                let t = (frame_idx + k) as f32 / IN_RATE as f32;
                let s = (std::f32::consts::TAU * FREQ * t).sin();
                callback.push(s); // FL
                callback.push(-s); // FR
                for c in 2..IN_CH {
                    callback.push(c as f32); // junk in unused channels
                }
            }
            self::super::capture::push_stereo(
                &callback,
                IN_CH,
                |s| s,
                &mut scratch,
                &mut prod,
                &overruns,
            );
            frame_idx += n;

            // Network thread side: drain like the send loop does.
            loop {
                let count = cons.pop_slice(&mut temp);
                if count == 0 {
                    break;
                }
                resampler.process(&temp[..count], &mut resampled);
                self::super::encoder::encode_f32_to_pcm_i16_le(&resampled, &mut payload);
                wire.extend_from_slice(&payload);
            }
        }

        assert_eq!(overruns.load(Ordering::Relaxed), 0, "ring overran");
        assert_eq!(
            wire.len() % 4,
            0,
            "wire must hold whole s16le stereo frames"
        );

        // Decode the wire as the receiver (Snapserver at 48000:16:2) would.
        let mut decoded: Vec<f32> = Vec::new();
        self::super::decoder::decode_pcm_i16_le_to_f32(&wire, &mut decoded);
        let wire_frames = decoded.len() / 2;
        let expected_frames = total_frames as f64 * WIRE_RATE as f64 / IN_RATE as f64;
        let len_err = (wire_frames as f64 - expected_frames).abs() / expected_frames;
        assert!(
            len_err < 0.005,
            "wire duration off: {wire_frames} frames vs ~{expected_frames}"
        );

        // Pitch check: zero crossings of L over the whole stream at 48k must
        // still be ~440Hz (2 crossings per cycle). A chipmunk regression
        // (channel/rate misinterpretation) blows way past the 2% tolerance.
        let left: Vec<f32> = decoded.chunks_exact(2).map(|f| f[0]).collect();
        let crossings = left
            .windows(2)
            .filter(|p| (p[0] <= 0.0 && p[1] > 0.0) || (p[0] >= 0.0 && p[1] < 0.0))
            .count();
        let expected_crossings = (2.0 * FREQ * SECONDS) as f64;
        let pitch_err = (crossings as f64 - expected_crossings).abs() / expected_crossings;
        assert!(
            pitch_err < 0.02,
            "pitch shifted: {crossings} crossings vs ~{expected_crossings}"
        );

        // Channel integrity: R must stay the negation of L (junk channels and
        // interleave shifts would break this immediately).
        for (i, f) in decoded.chunks_exact(2).enumerate().skip(8) {
            assert!(
                (f[0] + f[1]).abs() < 2e-2,
                "frame {i}: L={} R={} not mirrored",
                f[0],
                f[1]
            );
        }
    }
}
