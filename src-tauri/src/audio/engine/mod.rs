//! Audio engine: device selection, capture, and (later) buffering.

pub mod buffer;
pub mod capture;
pub mod device;
pub mod encoder;
pub mod latency;

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
    app_handle: AppHandle,
) -> Result<(cpal::Stream, StreamStats), String> {

    emit_log(
        &app_handle,
        "debug",
        format!(
            "Init stream: Mode={} Protocol=TCP, Device='{}', Rate={}, RingMs={}, Priority={}, DSCP={}, Format={}, Chunk={}, Loopback={}, AdaptiveBuf={} ({}ms-{}ms)",
            if is_server { "SERVER" } else { "CLIENT" }, device_name, sample_rate, ring_buffer_duration_ms, high_priority, dscp_strategy, format, chunk_size, is_loopback, enable_adaptive_buffer, min_buffer_ms, max_buffer_ms
        ),
    );

    let host = cpal::default_host();

    // Device selection logic
    let device = if is_loopback {
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

    // 0.5. Note: We deliberately DO NOT override the requested sample rate natively anymore.
    // If the user requests 48000Hz and the device natively captures at 44100Hz, we trust
    // the OS audio stack (PipeWire, PulseAudio, CoreAudio, WASAPI shared mode) to perform
    // the necessary resampling. Overriding this breaks protocol synchronization
    // with the downstream audio receiver.

    // Detect supported audio formats from device
    let supported_configs: Vec<_> = if is_loopback {
        device.supported_output_configs().map_err(|e| e.to_string())?.collect()
    } else {
        device.supported_input_configs().map_err(|e| e.to_string())?.collect()
    };

    use self::device::{pick_best, ConfigCandidate, SampleFmt};

    let to_fmt = |f: cpal::SampleFormat| match f {
        cpal::SampleFormat::F32 => Some(SampleFmt::F32),
        cpal::SampleFormat::I16 => Some(SampleFmt::I16),
        cpal::SampleFormat::U16 => Some(SampleFmt::U16),
        _ => None,
    };

    let candidates: Vec<ConfigCandidate> = supported_configs
        .iter()
        .filter_map(|r| {
            to_fmt(r.sample_format()).map(|format| ConfigCandidate {
                channels: r.channels(),
                format,
                min_rate: r.min_sample_rate().0,
                max_rate: r.max_sample_rate().0,
            })
        })
        .collect();

    let (config_range, selected_format) = match pick_best(&candidates, sample_rate) {
        Some(i) => {
            let cand = candidates[i];
            let range = supported_configs
                .iter()
                .find(|r| {
                    to_fmt(r.sample_format()) == Some(cand.format)
                        && r.channels() == cand.channels
                        && r.min_sample_rate().0 == cand.min_rate
                        && r.max_sample_rate().0 == cand.max_rate
                })
                .copied()
                .expect("candidate originates from supported_configs");
            (range, range.sample_format())
        }
        None => {
            let fallback = *supported_configs
                .first()
                .ok_or_else(|| "No supported audio config found".to_string())?;
            emit_log(
                &app_handle,
                "warn",
                format!(
                    "Requested Sample Rate {}Hz not natively supported. Relying on OS resampler.",
                    sample_rate
                ),
            );
            (fallback, fallback.sample_format())
        }
    };
    let device_channels = config_range.channels();

    emit_log(
        &app_handle,
        "info",
        format!("Audio Input: format={:?}, channels={}, rate={}Hz", selected_format, device_channels, sample_rate),
    );

    let stream_config = cpal::StreamConfig {
        channels: device_channels,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: self::capture::resolve_buffer_size(
            buffer_size,
            config_range.buffer_size(),
        ),
    };

    emit_log(&app_handle, "info", format!(
        "Capture buffer: requested {} frames -> {:?}",
        buffer_size, stream_config.buffer_size
    ));

    // 1. Setup Ring Buffer (latency profile drives the sizes; "custom" uses the
    // user's manual fields).
    let lp = if latency_profile == "custom" {
        self::latency::LatencyParams {
            ring_ms: ring_buffer_duration_ms,
            adaptive_min_ms: min_buffer_ms,
            adaptive_max_ms: max_buffer_ms,
            chunk_size,
            prefill_ms: ring_buffer_duration_ms,
        }
    } else {
        self::latency::params(&latency_profile, is_loopback)
    };
    let effective_chunk = lp.chunk_size;

    let ring_capacity_ms = lp.adaptive_max_ms.max(lp.ring_ms);
    let ring_buffer_size =
        (sample_rate as usize) * (device_channels as usize) * (ring_capacity_ms as usize) / 1000;

    let adj_ms_u32 = lp.ring_ms;
    let buffer_size_mb = (ring_buffer_size * std::mem::size_of::<f32>()) as f32 / (1024.0 * 1024.0);

    emit_log(
        &app_handle,
        "info",
        format!(
            "Ring buffer: {}ms ({:.2}MB) - Device type: {}",
            adj_ms_u32,
            buffer_size_mb,
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
    let sample_rate_clone = sample_rate;
    let is_server_clone = is_server;
    let auto_reconnect_net = auto_reconnect;
    let device_channels_net = device_channels;
    let dscp_clone = dscp_strategy.clone();
    let overruns_net = overruns.clone();
    let underruns_net = underruns.clone();
    let effective_chunk_net = effective_chunk;

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


        let mut temp_buffer = vec![0.0f32; effective_chunk_net as usize * device_channels_net as usize];
        let start_time = Instant::now();
        let mut last_stats_emit = Instant::now();

        // Quality tracking
        let mut last_quality_emit = Instant::now();

        let adaptive_min_ms = lp.adaptive_min_ms;

        let mut adaptive = self::buffer::AdaptiveBuffer::new(
            adaptive_min_ms,
            lp.adaptive_max_ms.max(adaptive_min_ms),
            super::constants::ADAPTIVE_BUFFER_STEP_MS,
            adj_ms_u32,
            6,
        );
        let mut glitch_baseline: u64 = 0;

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
        let prefill_ms = adaptive.target_ms();
        let prefill_samples =
            sample_rate as usize * device_channels_net as usize * prefill_ms as usize / 1000;
        emit_log(
            &app_handle_net,
            "info",
            format!(
                "Buffering... waiting for {} samples (200ms)",
                prefill_samples
            ),
        );

        let mut use_chunked = false;
        let mut wav_header_sent = false;
        let mut prefill_debug_timer = Instant::now();
        let mut payload: Vec<u8> = Vec::new();

        while is_running_clone.load(Ordering::Relaxed) {
            // 1. Hardware-Driven Pacing
            let current_buffered = cons.len();
            let min_chunk_samples = effective_chunk_net as usize * device_channels_net as usize;

            if current_stream.is_none() && current_buffered > 0 {
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
                    // Short outage: drain gradually
                    let drain_amount = (sample_rate as usize * device_channels_net as usize / 10).min(current_buffered);
                    let mut drain_buffer = vec![0.0f32; drain_amount];
                    let _ = cons.pop_slice(&mut drain_buffer);
                }
                thread::sleep(Duration::from_millis(10));
                
            } else if current_stream.is_some() && current_buffered < min_chunk_samples && prefilled {
                underruns_net.fetch_add(1, Ordering::Relaxed);
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
                // - "wav": Header + PCM (Browsers/VLC)
                // - "pcm": Raw PCM (audio receiver)
                if is_server_clone && format_clone == "wav" && !wav_header_sent {
                     {
                         // Send header with 0 length (handled by helper) for unknown/stream
                         let header = create_wav_header(sample_rate_clone, device_channels_net, 16);
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

                let count = cons.pop_slice(&mut temp_buffer);

                if count > 0 {
                    self::encoder::encode_f32_to_pcm_i16_le(&temp_buffer[..count], &mut payload);

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
    let capacity_hint = effective_chunk as usize * device_channels as usize;
    let audio_stream = self::capture::build_input_stream(
        &device,
        &stream_config,
        selected_format,
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
