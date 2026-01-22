use chrono::Local;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::{debug, error, info, trace, warn};
use ringbuf::HeapRb;
use serde::Serialize;
use socket2::{Domain, Protocol, Socket, TcpKeepalive, Type};
use std::io::Write;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use thread_priority::{ThreadBuilder, ThreadPriority};

// Log Event
#[derive(Clone, Serialize)]
struct LogEvent {
    timestamp: String,
    level: String,
    message: String,
}

#[derive(Clone, Serialize)]
struct QualityEvent {
    score: u8,          // 0-100
    jitter: f32,        // milliseconds
    avg_latency: f32,   // milliseconds
    buffer_health: f32, // 0.0-1.0
    error_count: u64,
}

#[derive(Clone, Serialize)]
struct StatsEvent {
    uptime_seconds: u64,
    bytes_sent: u64,
    bitrate_kbps: f64,
}

#[derive(Clone, Serialize)]
struct BufferResizeEvent {
    new_size_ms: u32,
    reason: String,
}

// Helper function to emit log events
fn emit_log(app: &AppHandle, level: &str, message: String) {
    // Also log to standard logger (terminal/file)
    match level {
        "error" => error!("{}", message),
        "warning" => warn!("{}", message),
        "info" => info!("{}", message),
        "debug" => debug!("{}", message),
        "trace" => trace!("{}", message),
        "success" => info!("SUCCESS: {}", message), // Map success to info
        _ => info!("[{}] {}", level, message),
    }

    let log = LogEvent {
        timestamp: Local::now().format("%H:%M:%S").to_string(),
        level: level.to_string(),
        message,
    };
    let _ = app.emit("log-event", log);
}

// Statistics tracker
struct StreamStats {
    #[allow(dead_code)]
    bytes_sent: Arc<AtomicU64>,
    #[allow(dead_code)]
    start_time: Instant,
    is_running: Arc<AtomicBool>,
}

impl Drop for StreamStats {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
    }
}

enum AudioCommand {
    Start {
        device_name: String,

        ip: String,
        port: u16,
        sample_rate: u32,
        buffer_size: u32,
        ring_buffer_duration_ms: u32,
        auto_reconnect: bool,
        high_priority: bool,
        dscp_strategy: String,
        chunk_size: u32,
        is_loopback: bool,
        enable_adaptive_buffer: bool,
        min_buffer_ms: u32,
        max_buffer_ms: u32,
        app_handle: AppHandle,
    },
    Stop,
}

pub struct AudioState {
    tx: Mutex<mpsc::Sender<AudioCommand>>,
}

impl AudioState {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut _current_stream_handle: Option<(cpal::Stream, StreamStats)> = None;
            let mut _reconnect_handle: Option<thread::JoinHandle<()>> = None;
            let should_reconnect = Arc::new(AtomicBool::new(false));

            // Keep track of current params for reconnection
            // (device_name, ip, port, sample_rate, buffer_size, ring_buffer_duration_ms, auto_reconnect, high_priority, dscp_strategy, chunk_size, silence_threshold, silence_timeout_seconds, is_loopback, enable_adaptive_buffer, min_buffer_ms, max_buffer_ms, enable_spin_strategy, app_handle)
            let mut _current_params: Option<(
                String,
                String,
                u16,
                u32,
                u32,
                u32,
                bool,
                bool,
                String,
                u32,
                bool,
                bool,
                u32,
                u32,
                AppHandle,
            )> = None;

            for command in rx {
                match command {
                    AudioCommand::Start {
                        device_name,
                        ip,
                        port,
                        sample_rate,
                        buffer_size,
                        ring_buffer_duration_ms,
                        auto_reconnect,
                        high_priority,
                        dscp_strategy,
                        chunk_size,
                        is_loopback,
                        enable_adaptive_buffer,
                        min_buffer_ms,
                        max_buffer_ms,
                        app_handle,
                    } => {
                        // Stop existing stream if any
                        if let Some((stream, stats)) = _current_stream_handle.take() {
                            drop(stream);
                            drop(stats); // Signals stats thread to stop
                        }

                        // Store params for reconnection
                        _current_params = Some((
                            device_name.clone(),
                            ip.clone(),
                            port,
                            sample_rate,
                            buffer_size,
                            ring_buffer_duration_ms,
                            auto_reconnect,
                            high_priority,
                            dscp_strategy.clone(),
                            chunk_size,
                            is_loopback,
                            enable_adaptive_buffer,
                            min_buffer_ms,
                            max_buffer_ms,
                            app_handle.clone(),
                        ));
                        should_reconnect.store(auto_reconnect, Ordering::Relaxed);

                        emit_log(
                            &app_handle,
                            "info",
                            format!("Starting stream to {}:{}", ip, port),
                        );

                        match start_audio_stream(
                            device_name,
                            ip,
                            port,
                            sample_rate,
                            buffer_size,
                            ring_buffer_duration_ms,
                            high_priority,
                            dscp_strategy,
                            chunk_size,
                            is_loopback,
                            enable_adaptive_buffer,
                            min_buffer_ms,
                            max_buffer_ms,
                            app_handle.clone(),
                        ) {
                            Ok((stream, stats)) => {
                                stream.play().unwrap();
                                _current_stream_handle = Some((stream, stats));
                                emit_log(
                                    &app_handle,
                                    "success",
                                    "Stream started successfully".to_string(),
                                );
                            }
                            Err(e) => {
                                emit_log(
                                    &app_handle,
                                    "error",
                                    format!("Failed to start stream: {}", e),
                                );
                                // Trigger reconnect logic if enabled
                                if auto_reconnect {
                                    // This would need more complex logic to retry,
                                    // for now we just log error.
                                    // In a real implementation, we'd loop here.
                                }
                            }
                        }
                    }
                    AudioCommand::Stop => {
                        should_reconnect.store(false, Ordering::Relaxed);
                        _current_stream_handle = None;
                        _current_params = None;
                        // We can't easily access the app_handle here to log "Stopped" unless we stored it in the struct or passed it.
                        // But the frontend knows it called stop.
                    }
                }

                // Reconnection check (simple polling for now, ideally would be event driven)
                if should_reconnect.load(Ordering::Relaxed) && _current_stream_handle.is_none() {
                    if let Some((
                        device_name,
                        ip,
                        port,
                        sample_rate,
                        buffer_size,
                        ring_buffer_duration_ms,
                        _auto_reconnect,
                        high_priority,
                        dscp_strategy,
                        chunk_size,
                        is_loopback,
                        enable_adaptive_buffer,
                        min_buffer_ms,
                        max_buffer_ms,
                        app_handle,
                    )) = &_current_params
                    {
                        // Try to reconnect
                        // In a real app, we'd want a delay here to avoid tight loops
                        thread::sleep(Duration::from_secs(2));

                        emit_log(app_handle, "info", "Attempting to reconnect...".to_string());

                        match start_audio_stream(
                            device_name.clone(),
                            ip.clone(),
                            port.clone(),
                            sample_rate.clone(),
                            buffer_size.clone(),
                            ring_buffer_duration_ms.clone(),
                            high_priority.clone(),
                            dscp_strategy.clone(),
                            chunk_size.clone(),
                            is_loopback.clone(),
                            enable_adaptive_buffer.clone(),
                            min_buffer_ms.clone(),
                            max_buffer_ms.clone(),
                            app_handle.clone(),
                        ) {
                            Ok((stream, stats)) => {
                                stream.play().unwrap();
                                _current_stream_handle = Some((stream, stats));
                                emit_log(
                                    app_handle,
                                    "success",
                                    "Reconnected successfully".to_string(),
                                );
                            }
                            Err(e) => {
                                emit_log(
                                    app_handle,
                                    "warning",
                                    format!("Reconnection failed: {}", e),
                                );
                            }
                        }
                    }
                }
            }
        });

        Self { tx: Mutex::new(tx) }
    }

    pub fn shutdown(&self) {
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(AudioCommand::Stop);
        }
    }
}

/// Gracefully close a TCP stream, ensuring FIN is sent to prevent zombie connections
fn close_tcp_stream(stream: TcpStream, context: &str, app_handle: &AppHandle) {
    use std::net::Shutdown;

    // Send TCP FIN to server (graceful shutdown)
    if let Err(e) = stream.shutdown(Shutdown::Both) {
        // May fail if already closed, which is fine
        emit_log(
            app_handle,
            "debug",
            format!(
                "TCP shutdown {} ({}): socket may already be closed",
                context, e
            ),
        );
    } else {
        emit_log(
            app_handle,
            "debug",
            format!("TCP connection closed gracefully ({})", context),
        );
    }

    // stream drops here, releasing the socket
}

fn start_audio_stream(
    device_name: String,

    ip: String,
    port: u16,
    sample_rate: u32,
    buffer_size: u32,
    ring_buffer_duration_ms: u32,
    high_priority: bool,
    dscp_strategy: String,
    chunk_size: u32,
    is_loopback: bool,
    enable_adaptive_buffer: bool,
    min_buffer_ms: u32,
    max_buffer_ms: u32,
    app_handle: AppHandle,
) -> Result<(cpal::Stream, StreamStats), String> {
    emit_log(
        &app_handle,
        "debug",
        format!(
            "Init stream: Device='{}', Rate={}, Buf={}, RingMs={}, Priority={}, DSCP={}, Chunk={}, Loopback={}, AdaptiveBuf={} ({}ms-{}ms)",
            device_name, sample_rate, buffer_size, ring_buffer_duration_ms, high_priority, dscp_strategy, chunk_size, is_loopback, enable_adaptive_buffer, min_buffer_ms, max_buffer_ms
        ),
    );

    let host = cpal::default_host();

    // Device selection logic
    let device = if is_loopback {
        // Loopback mode: Search in OUTPUT devices
        // Remove "[Loopback] " prefix if present for matching
        let clean_name = device_name.replace("[Loopback] ", "");
        let mut found_device = None;

        let available_devices = host.output_devices().map_err(|e| e.to_string())?;
        let mut device_list_log = Vec::new();

        for dev in available_devices {
            if let Ok(name) = dev.name() {
                device_list_log.push(name.clone());
                // Try exact match or match without whitespace
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

    // 1. Setup Ring Buffer with Smart Sizing
    // Adjust ring buffer duration based on device type for network-aware buffering
    let adjusted_ring_buffer_duration_ms = if is_loopback {
        // WASAPI Loopback: Use larger default (8000ms) to handle:
        // - WiFi jitter (50-100ms)
        // - Laptop CPU throttling (20-50ms)
        // - WASAPI loopback timing unpredictability
        8000.max(ring_buffer_duration_ms)
    } else {
        // Standard Input/VB Cable: Use 5000ms default for WiFi tolerance
        5000.max(ring_buffer_duration_ms)
    };

    let ring_buffer_size =
        (sample_rate as usize) * 2 * (adjusted_ring_buffer_duration_ms as usize) / 1000;
    let buffer_size_mb = (ring_buffer_size * 2) as f32 / (1024.0 * 1024.0);

    emit_log(
        &app_handle,
        "info",
        format!(
            "Ring buffer: {}ms ({:.2}MB) - Device type: {}",
            adjusted_ring_buffer_duration_ms,
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

    let bytes_sent_clone = bytes_sent.clone();
    let is_running_clone = is_running.clone();
    let app_handle_net = app_handle.clone();
    let ip_clone = ip.clone();

    // 2. Spawn Network Thread (Consumer)
    // Use ThreadBuilder to set priority
    let priority = if high_priority {
        ThreadPriority::Max
    } else {
        ThreadPriority::Min
    }; // Min is usually normal/default
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
        } else {
            if high_priority {
                emit_log(
                    &app_handle_net,
                    "info",
                    "Network thread priority set to Max".to_string(),
                );
            }
        }
        let server_addr = format!("{}:{}", ip_clone, port);
        let mut sequence: u32 = 0;
        // Dynamic chunk size
        let mut temp_buffer = vec![0.0f32; chunk_size as usize];
        let _dropped_packets: u64 = 0;
        let start_time = Instant::now();
        let mut last_stats_emit = Instant::now();

        // Quality tracking variables
        let mut latency_samples: Vec<f32> = Vec::with_capacity(100); //  Track last 100
        let mut jitter_avg: f32 = 0.0; // EWMA of jitter
        let mut _last_write_time: Option<Instant> = None;
        let consecutive_errors: u64 = 0;
        let mut last_quality_emit = Instant::now();

        // Adaptive buffer tracking with network-aware ranges
        let mut current_buffer_ms = adjusted_ring_buffer_duration_ms;
        let mut last_buffer_check = Instant::now();
        const BUFFER_CHECK_INTERVAL_SECS: u64 = 10; // Check every 10 seconds

        // Adjust adaptive buffer ranges based on device type
        let (adaptive_min_ms, adaptive_max_ms) = if is_loopback {
            // WASAPI Loopback: 4000-12000ms range
            (4000.max(min_buffer_ms), 12000.min(max_buffer_ms))
        } else {
            // Standard Input: 2000-6000ms range
            (2000.max(min_buffer_ms), 6000.min(max_buffer_ms))
        };

        // Exponential backoff for reconnection
        let mut retry_delay = Duration::from_secs(1);
        const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);

        // We wrap the stream in an Option to handle reconnection
        let mut current_stream: Option<TcpStream> = None;

        emit_log(
            &app_handle_net,
            "info",
            "Network thread started".to_string(),
        );

        // Heartbeat tracking for detecting silent failures
        let mut last_heartbeat = Instant::now();
        const HEARTBEAT_INTERVAL_SECS: u64 = 30;

        // Pacer State variables removed (Strict Clock Strategy used)

        // --- PREFILL GATE: Wait for buffer to fill before starting transmission ---
        // This prevents "cold start" stuttering by ensuring we have a cushion of data (1000ms).
        // Works for Windows, Linux, and macOS equally.
        let prefill_samples = sample_rate as usize * 1; // 1 second (1000ms) of audio
        emit_log(
            &app_handle_net,
            "info",
            format!(
                "Buffering... waiting for {} samples (1000ms)",
                prefill_samples
            ),
        );

        while cons.len() < prefill_samples && is_running_clone.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(10));
        }

        emit_log(
            &app_handle_net,
            "success",
            "Buffer prefilled! Starting transmission.".to_string(),
        );
        // --------------------------------------------------------------------------

        // Strict Pacing Setup
        let tick_duration =
            Duration::from_micros((chunk_size as u64 * 1_000_000) / sample_rate as u64);
        let mut next_tick = Instant::now();

        while is_running_clone.load(Ordering::Relaxed) {
            // 1. Strict Pacing with Adaptive Drain
            // Check buffer health. If we have too much data, we are drifting behind the source.
            // Catch up by skipping sleep.
            let now = Instant::now();
            let current_buffered = cons.len();
            // High Water Mark: Prefill + ~100ms (sample_rate / 10).
            // We want to keep buffer close to prefill level.
            let high_water_mark = prefill_samples + (sample_rate as usize / 10);

            if current_buffered > high_water_mark {
                // DRAIN MODE: We are lagging. Process immediately.
                // Reset next_tick to avoid accumulating "debt" and to return to strict pacing smoothly later.
                next_tick = now + tick_duration;
            } else {
                // STRICT MODE: Respect the clock.
                if now < next_tick {
                    thread::sleep(next_tick - now);
                    next_tick += tick_duration;
                } else {
                    // We are visibly behind schedule (OS lag?)
                    // If massive lag (>200ms), reset clock.
                    if now.duration_since(next_tick) > Duration::from_millis(200) {
                        next_tick = Instant::now() + tick_duration;
                    } else {
                        next_tick += tick_duration;
                    }
                }
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

            // 2. Fetch or Generate Silence (Non-blocking)
            let required_samples = temp_buffer.len();
            let count = if cons.len() >= required_samples {
                cons.pop_slice(&mut temp_buffer)
            } else {
                // Starvation: Send silence (zeros)
                temp_buffer.fill(0.0);
                // Log starvation occasionally to avoid spamming
                static LAST_STARVATION_LOG: AtomicU64 = AtomicU64::new(0);
                let now_millis = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                if now_millis - LAST_STARVATION_LOG.load(Ordering::Relaxed) > 5000 {
                    emit_log(
                        &app_handle_net,
                        "debug",
                        format!(
                            "Starvation: Generating silence chunk (Buffer < {})",
                            required_samples
                        ),
                    );
                    LAST_STARVATION_LOG.store(now_millis, Ordering::Relaxed);
                }
                required_samples
            };

            // 3. Connection Management (Reconnection or Auto-Disconnect)
            if current_stream.is_none() {
                // Otherwise, we have audio (or short silence), so we try to connect
                emit_log(
                    &app_handle_net,
                    "warning",
                    format!(
                        "Reconnecting to {} (retry in {}s)...",
                        server_addr,
                        retry_delay.as_secs()
                    ),
                );

                // Wait before attempting reconnection (exponential backoff)
                thread::sleep(retry_delay);

                // Advanced Socket Setup using socket2
                let connect_result = (|| -> Result<TcpStream, Box<dyn std::error::Error>> {
                    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;

                    socket.set_send_buffer_size(1024 * 1024)?;
                    // TCP Keepalive
                    let keepalive = TcpKeepalive::new()
                        .with_time(Duration::from_secs(5))
                        .with_interval(Duration::from_secs(2));
                    socket.set_tcp_keepalive(&keepalive)?;

                    // TOS / DSCP
                    let tos_value = match dscp_strategy.as_str() {
                        "voip" => 0xB8,       // EF
                        "lowdelay" => 0x10,   // IPTOS_LOWDELAY
                        "throughput" => 0x08, // IPTOS_THROUGHPUT
                        "besteffort" => 0x00,
                        _ => 0xB8, // Default to EF
                    };
                    let _ = socket.set_tos(tos_value);

                    let addr: SocketAddr = server_addr.parse()?;
                    socket.connect(&addr.into())?;

                    let stream: TcpStream = socket.into();
                    stream.set_nodelay(true)?;

                    // Write Timeout (Critical)
                    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
                    emit_log(
                        &app_handle_net,
                        "trace",
                        "Write timeout set to 5s".to_string(),
                    );

                    Ok(stream)
                })();

                match connect_result {
                    Ok(s) => {
                        retry_delay = Duration::from_secs(1);
                        current_stream = Some(s);
                        emit_log(
                            &app_handle_net,
                            "success",
                            "Connected successfully!".to_string(),
                        );
                        // Important: logic falls through to Sending block
                    }
                    Err(e) => {
                        emit_log(
                            &app_handle_net,
                            "error",
                            format!("Reconnection failed: {}", e),
                        );
                        retry_delay = std::cmp::min(retry_delay * 2, MAX_RETRY_DELAY);
                        continue;
                    }
                }
            }

            // 4. Sending Logic
            if let Some(ref mut s) = current_stream {
                // Inner pacer removed. Using strict clock at top.
                // let chunk_frames = count as u64 / 2; // Unused

                // Send
                let data_len = (count * 2) as u32;
                let mut payload = Vec::with_capacity(data_len as usize);
                for i in 0..count {
                    // CONVERT F32 -> I16 (Network Edge)
                    let sample = temp_buffer[i].clamp(-1.0, 1.0);
                    let sample_i16 = (sample * 32767.0) as i16;
                    payload.extend_from_slice(&sample_i16.to_le_bytes());
                }

                // Jitter Calculation (Deviation from schedule)
                let send_time = Instant::now();
                // Target was the tick start time (next_tick - duration)
                let target = next_tick - tick_duration;
                let deviation_ms = if send_time > target {
                    send_time.duration_since(target).as_secs_f32() * 1000.0
                } else {
                    0.0
                };

                // Exponential Weighted Moving Average (EWMA) for Jitter
                // Alpha 0.1 gives 10-sample smoothing
                if jitter_avg == 0.0 {
                    jitter_avg = deviation_ms;
                } else {
                    jitter_avg = 0.9 * jitter_avg + 0.1 * deviation_ms;
                }

                let write_start = Instant::now();
                if let Err(e) = s.write_all(&payload) {
                    emit_log(&app_handle_net, "error", format!("Write error: {}", e));
                    current_stream = None;
                } else {
                    // Update Latency (Network Write Time)
                    let write_duration_ms = write_start.elapsed().as_secs_f32() * 1000.0;
                    if latency_samples.len() >= 100 {
                        latency_samples.remove(0);
                    }
                    latency_samples.push(write_duration_ms);

                    let _ = bytes_sent_clone.fetch_add(payload.len() as u64, Ordering::Relaxed);
                    sequence = sequence.wrapping_add(1);
                    _last_write_time = Some(Instant::now());
                }
            }

            // Emit stats every second regardless of audio activity
            if last_stats_emit.elapsed() >= Duration::from_secs(1) {
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

            // Emit quality metrics every 2 seconds
            if last_quality_emit.elapsed() >= Duration::from_secs(2) {
                // Calculate average latency
                let avg_latency = if !latency_samples.is_empty() {
                    latency_samples.iter().sum::<f32>() / latency_samples.len() as f32
                } else {
                    0.0
                };

                // Get buffer health from the most recent buffer usage check
                let occupied = cons.len();
                let capacity = cons.capacity();
                let buffer_health = 1.0 - (occupied as f32 / capacity as f32);

                // Calculate quality score (0-100)
                // Jitter penalty: 0-50 points (lower is better)
                // Target jitter < 5ms = 0 penalty, >20ms = max penalty
                let jitter_penalty = ((jitter_avg / 20.0).min(1.0) * 50.0) as u8;

                // Buffer penalty: 0-30 points (lower usage = better)
                // Usage > 80% = max penalty, < 50% = no penalty
                let buffer_usage = occupied as f32 / capacity as f32;
                let buffer_penalty = if buffer_usage > 0.8 {
                    ((buffer_usage - 0.8) / 0.2 * 30.0) as u8
                } else {
                    0
                };

                // Error penalty: 0-20 points
                // More than 5 consecutive errors = max penalty
                let error_penalty = ((consecutive_errors.min(5) as f32 / 5.0) * 20.0) as u8;

                let score = 100u8.saturating_sub(jitter_penalty + buffer_penalty + error_penalty);

                let _ = app_handle_net.emit(
                    "quality-event",
                    QualityEvent {
                        score,
                        jitter: jitter_avg,
                        avg_latency,
                        buffer_health,
                        error_count: consecutive_errors,
                    },
                );
                last_quality_emit = Instant::now();
            }

            // Adaptive buffer sizing - check periodically
            if enable_adaptive_buffer
                && last_buffer_check.elapsed() >= Duration::from_secs(BUFFER_CHECK_INTERVAL_SECS)
            {
                // Determine target buffer size based on jitter (using network-aware ranges)
                let target_buffer_ms = if jitter_avg < 5.0 {
                    // Low jitter - can use smaller buffer
                    adaptive_min_ms
                } else if jitter_avg > 15.0 {
                    // High jitter - need larger buffer
                    adaptive_max_ms
                } else {
                    // Medium jitter scale linearly between min and max
                    // jitter 5-15ms maps to min-max buffer
                    let jitter_ratio = (jitter_avg - 5.0) / 10.0; // 0.0 to 1.0
                    let buffer_range = adaptive_max_ms - adaptive_min_ms;
                    adaptive_min_ms + (buffer_range as f32 * jitter_ratio) as u32
                };

                // Only resize if the change is significant (>10% difference)
                let size_diff_pct = ((target_buffer_ms as f32 - current_buffer_ms as f32).abs()
                    / current_buffer_ms as f32)
                    * 100.0;

                if size_diff_pct > 10.0 {
                    let reason = if target_buffer_ms > current_buffer_ms {
                        format!("Increased due to high jitter ({:.1}ms)", jitter_avg)
                    } else {
                        format!("Decreased due to low jitter ({:.1}ms)", jitter_avg)
                    };

                    emit_log(
                        &app_handle_net,
                        "info",
                        format!(
                            "Adaptive buffer: {}ms → {}ms (jitter: {:.1}ms, range: {}-{}ms). {}",
                            current_buffer_ms,
                            target_buffer_ms,
                            jitter_avg,
                            adaptive_min_ms,
                            adaptive_max_ms,
                            reason
                        ),
                    );

                    let _ = app_handle_net.emit(
                        "buffer-resize-event",
                        BufferResizeEvent {
                            new_size_ms: target_buffer_ms,
                            reason: reason.clone(),
                        },
                    );

                    current_buffer_ms = target_buffer_ms;

                    // Note: Actual ring buffer resizing would require complex synchronization
                    // to avoid audio dropouts. For now, we just track the target size and
                    // emit events. Full implementation would need to:
                    // 1. Create new ring buffer with new size
                    // 2. Copy existing data to new buffer
                    // 3. Atomically swap prod/cons references
                    // This is marked as future enhancement.
                }

                last_buffer_check = Instant::now();
            }
        }

        // Close TCP connection gracefully before thread exits
        if let Some(stream) = current_stream.take() {
            close_tcp_stream(stream, "thread exit", &app_handle_net);
        }

        // Log why network thread is exiting
        let exit_reason = if !is_running_clone.load(Ordering::Relaxed) {
            "User stopped stream"
        } else {
            "Unexpected termination - is_running flag false"
        };

        emit_log(
            &app_handle_net,
            "info",
            format!("Network thread stopped: {}", exit_reason),
        );
    });

    // 3. Setup Audio Stream (Producer)
    let app_handle_err = app_handle.clone();
    let err_fn = move |err| {
        // Enhanced error logging with more context
        emit_log(
            &app_handle_err,
            "error",
            format!(
                "⚠️ AUDIO STREAM ERROR: {} - This may cause stream to stop!",
                err
            ),
        );
    };

    // Define the data callback (closure)
    // We need to clone the necessary variables to move them into the closure
    // But since we might need to create the stream twice (retry logic), we need a way to create the callback twice.
    // However, the callback consumes 'prod' (the ring buffer producer), which is not Clone.
    // So we cannot easily retry by just calling build_input_stream twice with the same closure.
    // We would need to recreate the producer or wrap it in a Mutex/Arc, but that defeats the lock-free purpose.

    // ALTERNATIVE: We can try to build the stream with the config FIRST, and if it fails, try another config.
    // But build_input_stream takes the callback.

    // Solution: Since we can't clone the producer, we will try to clone the config and check support BEFORE building?
    // Or we can just use BufferSize::Default for Loopback ALWAYS?
    // Loopback is sensitive. Fixed buffer size is often not supported.
    // Let's try to use Default buffer size for Loopback if Fixed is not strictly required.
    // The user's "Buffer Size" setting is technically "Hardware Latency".
    // If we use Default, we get whatever the system gives (usually 10ms).

    // Configure WASAPI buffer: use Default for loopback (more compatible)
    // Fixed buffer size often fails with loopback, so we use Default and rely on
    // the larger ring buffer for stability
    // 4. Setup Ring Buffer (Existing code ends around here)

    // 5. Determine Stream Config (Dynamic Format)
    let (stream_config, selected_format) = {
        // Detect supported formats
        let supported_configs: Vec<_> = device
            .supported_input_configs()
            .map_err(|e| e.to_string())?
            .collect();
        let mut best_config_range = None;

        // Priority: F32 > I16 > U16 (F32 is preferred for quality and is native on PipeWire/Linux)
        for format in [
            cpal::SampleFormat::F32,
            cpal::SampleFormat::I16,
            cpal::SampleFormat::U16,
        ] {
            for range in &supported_configs {
                if range.sample_format() == format && range.channels() == 2 {
                    if range.min_sample_rate().0 <= sample_rate
                        && range.max_sample_rate().0 >= sample_rate
                    {
                        best_config_range = Some(range.clone());
                        break;
                    }
                }
            }
            if best_config_range.is_some() {
                break;
            }
        }

        // Fallback or Loopback-specific logic
        if best_config_range.is_none() {
            for range in &supported_configs {
                // Relaxed check: just matching rate and channels
                if range.channels() == 2
                    && range.min_sample_rate().0 <= sample_rate
                    && range.max_sample_rate().0 >= sample_rate
                {
                    best_config_range = Some(range.clone());
                    emit_log(
                        &app_handle,
                        "warning",
                        format!("Fallback format: {:?}", range.sample_format()),
                    );
                    break;
                }
            }
        }

        let config_range = best_config_range.ok_or_else(|| {
            format!(
                "No supported config found for 2 channels at {}Hz",
                sample_rate
            )
        })?;

        let selected_format = config_range.sample_format();
        emit_log(
            &app_handle,
            "info",
            format!("Detected Input Format: {:?}", selected_format),
        );

        // Use Default buffer for Loopback (compatibility) or Fixed for standard
        let buffer_size_setting = if is_loopback {
            cpal::BufferSize::Default
        } else {
            cpal::BufferSize::Fixed(buffer_size)
        };

        (
            cpal::StreamConfig {
                channels: 2,
                sample_rate: cpal::SampleRate(sample_rate),
                buffer_size: buffer_size_setting,
            },
            selected_format,
        )
    };

    // Shared producer (mutex wrapped for multiple closures - uncontended in practice)
    let prod = Arc::new(Mutex::new(prod));

    // 4. Define Audio Processing Function (Local Helper)
    // This avoids code duplication across F32/I16/U16 arms.
    // It takes a slice of NORMALIZED I16 samples.

    // State captured by closures. Ideally we'd put this in a struct,
    // but for simplicity we'll just instantiate the state variables inside each closure
    // effectively duplicating the "state machine" per stream, which is fine because only one stream runs.
    // WAIT: `process_audio_i16` needs access to `prod` and `app_handle`.
    // It also needs mut access to `smoothed_rms`, `signal_average`, `silence_start`, etc.
    // A clean way is to define a struct `AudioProcessor` that holds this state and has a `process` method.

    struct AudioProcessor {
        prod: Arc<Mutex<ringbuf::Producer<f32, Arc<ringbuf::HeapRb<f32>>>>>,
        app_handle: AppHandle,
    }

    impl AudioProcessor {
        fn new(
            prod: Arc<Mutex<ringbuf::Producer<f32, Arc<ringbuf::HeapRb<f32>>>>>,
            app_handle: AppHandle,
        ) -> Self {
            Self { prod, app_handle }
        }

        fn process(&mut self, data: &[f32]) {
            // Write to Ring Buffer
            if let Ok(mut guard) = self.prod.lock() {
                let pushed = guard.push_slice(data);
                if pushed < data.len() {
                    static LAST_OVERFLOW: AtomicU64 = AtomicU64::new(0);
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;
                    if now - LAST_OVERFLOW.load(Ordering::Relaxed) > 5000 {
                        emit_log(
                            &self.app_handle,
                            "warning",
                            format!("Buffer overflow: Dropped {} samples", data.len() - pushed),
                        );
                        LAST_OVERFLOW.store(now, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    // 5. Build Stream based on Format
    let audio_stream = match selected_format {
        cpal::SampleFormat::F32 => {
            let mut processor = AudioProcessor::new(prod.clone(), app_handle.clone());
            device.build_input_stream(
                &stream_config,
                move |data: &[f32], _: &_| {
                    // F32 -> F32 (Passthrough)
                    processor.process(data);
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let mut processor = AudioProcessor::new(prod.clone(), app_handle.clone());
            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _: &_| {
                    // I16 -> F32 (Normalize)
                    let mut converted = Vec::with_capacity(data.len());
                    for &sample in data {
                        let s_f32 = sample as f32 / 32768.0;
                        converted.push(s_f32);
                    }
                    processor.process(&converted);
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let mut processor = AudioProcessor::new(prod.clone(), app_handle.clone());
            device.build_input_stream(
                &stream_config,
                move |data: &[u16], _: &_| {
                    // U16 -> F32 (Normalize)
                    let mut converted = Vec::with_capacity(data.len());
                    for &sample in data {
                        // u16 is 0..65535. center is 32768.
                        // (s - 32768) / 32768.0
                        let s_f32 = (sample as f32 - 32768.0) / 32768.0;
                        converted.push(s_f32);
                    }
                    processor.process(&converted);
                },
                err_fn,
                None,
            )
        }
        _ => return Err(format!("Unsupported sample format: {:?}", selected_format)),
    }
    .map_err(|e| e.to_string())?;

    Ok((
        audio_stream,
        StreamStats {
            bytes_sent,
            start_time: Instant::now(),
            is_running,
        },
    ))
}

#[tauri::command]
pub fn get_input_devices(
    #[allow(unused_variables)] include_loopback: bool,
) -> Result<Vec<String>, String> {
    let mut all_devices = Vec::new();

    // Try all available hosts
    for host_id in cpal::available_hosts() {
        let host = cpal::host_from_id(host_id).map_err(|e| e.to_string())?;
        debug!("Scanning host: {:?}", host_id);

        // 1. Standard Input Devices
        if let Ok(devices) = host.input_devices() {
            for device in devices {
                if let Ok(name) = device.name() {
                    // Filter out ALSA virtual devices on Linux that don't work well
                    #[cfg(target_os = "linux")]
                    {
                        if name.contains("default")
                            || name.contains("sysdefault")
                            || name.contains("null")
                            || name.starts_with("hw:")
                        {
                            continue; // Skip this device
                        }
                    }

                    if !all_devices.contains(&name) {
                        all_devices.push(name);
                    }
                }
            }
        }

        // 2. Loopback Devices (Windows only)
        #[cfg(target_os = "windows")]
        if include_loopback {
            if let Ok(devices) = host.output_devices() {
                for device in devices {
                    if let Ok(name) = device.name() {
                        let loopback_name = format!("[Loopback] {}", name);
                        if !all_devices.contains(&loopback_name) {
                            all_devices.push(loopback_name);
                        }
                    }
                }
            }
        }
    }

    // Fallback to default host if empty
    if all_devices.is_empty() {
        let host = cpal::default_host();
        if let Ok(devices) = host.input_devices() {
            for device in devices {
                if let Ok(name) = device.name() {
                    // Filter out ALSA virtual devices on Linux that don't work well
                    #[cfg(target_os = "linux")]
                    {
                        if name.contains("default")
                            || name.contains("sysdefault")
                            || name.contains("null")
                            || name.starts_with("hw:")
                        {
                            continue; // Skip this device
                        }
                    }

                    if !all_devices.contains(&name) {
                        all_devices.push(name);
                    }
                }
            }
        }

        #[cfg(target_os = "windows")]
        if include_loopback {
            if let Ok(devices) = host.output_devices() {
                for device in devices {
                    if let Ok(name) = device.name() {
                        let loopback_name = format!("[Loopback] {}", name);
                        if !all_devices.contains(&loopback_name) {
                            all_devices.push(loopback_name);
                        }
                    }
                }
            }
        }
    }

    info!("Found devices: {:?}", all_devices);
    Ok(all_devices)
}

#[tauri::command]
pub async fn start_stream(
    state: tauri::State<'_, AudioState>,
    device_name: String,
    ip: String,
    port: u16,
    sample_rate: u32,
    buffer_size: u32,
    ring_buffer_duration_ms: u32,
    auto_reconnect: bool,
    high_priority: bool,
    dscp_strategy: String,
    chunk_size: u32,
    is_loopback: bool,
    enable_adaptive_buffer: bool,
    min_buffer_ms: u32,
    max_buffer_ms: u32,
    app_handle: AppHandle,
) -> Result<(), String> {
    let tx = state.tx.lock().map_err(|e| e.to_string())?;
    tx.send(AudioCommand::Start {
        device_name,
        ip,
        port,
        sample_rate,
        buffer_size,
        ring_buffer_duration_ms,
        auto_reconnect,
        high_priority,
        dscp_strategy,
        chunk_size,
        is_loopback,
        enable_adaptive_buffer,
        min_buffer_ms,
        max_buffer_ms,
        app_handle,
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn stop_stream(state: tauri::State<'_, AudioState>) -> Result<(), String> {
    let tx = state.tx.lock().map_err(|e| e.to_string())?;
    tx.send(AudioCommand::Stop).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_os_type() -> String {
    std::env::consts::OS.to_string()
}
