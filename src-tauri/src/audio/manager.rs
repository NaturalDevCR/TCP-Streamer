use super::stats::{emit_log, BufferResizeEvent, QualityEvent, StatsEvent, StreamStats};
use super::stream::StreamSocket;
use super::wav_helper::create_wav_header;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::error;

use ringbuf::HeapRb;
use socket2::{Domain, Protocol, Socket, TcpKeepalive, Type};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use thread_priority::{ThreadBuilder, ThreadPriority};

struct ChunkedWriter<W: Write> {
    writer: W,
}

impl<W: Write> ChunkedWriter<W> {
    fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: Write> Write for ChunkedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        write!(self.writer, "{:X}\r\n", buf.len())?;
        self.writer.write_all(buf)?;
        write!(self.writer, "\r\n")?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

pub enum AudioCommand {
    Start {
        device_name: String,
        // protocol removed
        ip: String,
        port: u16,
        sample_rate: u32,
        buffer_size: u32,
        ring_buffer_duration_ms: u32,
        auto_reconnect: bool,
        high_priority: bool,
        dscp_strategy: String,
        format: String, // "pcm" or "mp3"
        chunk_size: u32,
        is_loopback: bool,
        is_server: bool,
        enable_adaptive_buffer: bool,
        min_buffer_ms: u32,
        max_buffer_ms: u32,
        app_handle: AppHandle,
    },
    Stop,
}

pub struct AudioState {
    pub tx: Mutex<mpsc::Sender<AudioCommand>>,
}

impl AudioState {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut _current_stream_handle: Option<(cpal::Stream, StreamStats)> = None;
            let mut _reconnect_handle: Option<thread::JoinHandle<()>> = None;
            let should_reconnect = Arc::new(AtomicBool::new(false));

            // Keep track of current params for reconnection
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
                // protocol removed
                String,
                u32,
                bool,
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
                        // protocol removed
                        ip,
                        port,
                        sample_rate,
                        buffer_size,
                        ring_buffer_duration_ms,
                        auto_reconnect,
                        high_priority,
                        dscp_strategy,
                        format,
                        chunk_size,
                        is_loopback,
                        is_server,
                        enable_adaptive_buffer,
                        min_buffer_ms,
                        max_buffer_ms,
                        app_handle,
                    } => {
                        // Stop existing stream if any
                        if let Some((stream, stats)) = _current_stream_handle.take() {
                            stats.is_running.store(false, Ordering::Relaxed);
                            stream.pause().ok(); // Try to pause first
                            drop(stream);
                            drop(stats); // Signals stats thread to stop
                        }

                        // Store params for reconnection
                        _current_params = Some((
                            device_name.clone(),
                            // protocol removed
                            ip.clone(),
                            port,
                            sample_rate,
                            buffer_size,
                            ring_buffer_duration_ms,
                            auto_reconnect,
                            high_priority,
                            dscp_strategy.clone(),
                            if is_server { format.clone() } else { "pcm".to_string() },
                            chunk_size,
                            is_loopback,
                            is_server,
                            enable_adaptive_buffer,
                            min_buffer_ms,
                            max_buffer_ms,
                            app_handle.clone(),
                        ));
                        should_reconnect.store(auto_reconnect, Ordering::Relaxed);

                        if is_server {
                            emit_log(
                                &app_handle,
                                "info",
                                format!(
                                    "Starting TCP Server on port {}",
                                    port
                                ),
                            );
                        } else {
                            emit_log(
                                &app_handle,
                                "info",
                                format!(
                                    "Starting TCP stream to {}:{}",
                                    ip,
                                    port
                                ),
                            );
                        }

                        match start_audio_stream(
                            device_name,
                            // protocol removed
                            ip,
                            port,
                            sample_rate,
                            buffer_size,
                            ring_buffer_duration_ms,
                            high_priority,
                            dscp_strategy,
                            format,
                            chunk_size,
                            is_loopback,
                            is_server,
                            enable_adaptive_buffer,
                            min_buffer_ms,
                            max_buffer_ms,
                            app_handle.clone(),
                        ) {
                            Ok((stream, stats)) => {
                                if let Err(e) = stream.play() {
                                    emit_log(&app_handle, "error", format!("Failed to play stream: {}", e));
                                } else {
                                    _current_stream_handle = Some((stream, stats));
                                    emit_log(
                                        &app_handle,
                                        "success",
                                        "Stream started successfully".to_string(),
                                    );
                                }
                            }
                            Err(e) => {
                                emit_log(
                                    &app_handle,
                                    "error",
                                    format!("Failed to start stream: {}", e),
                                );
                            }
                        }
                    }
                    AudioCommand::Stop => {
                        should_reconnect.store(false, Ordering::Relaxed);
                        if let Some((stream, stats)) = _current_stream_handle.take() {
                             stats.is_running.store(false, Ordering::Relaxed);
                             stream.pause().ok();
                             drop(stream);
                        }
                        _current_stream_handle = None;
                        _current_params = None;
                    }
                }

                // Reconnection check
                if should_reconnect.load(Ordering::Relaxed) && _current_stream_handle.is_none() {
                    if let Some((
                        device_name,
                        // protocol removed
                        ip,
                        port,
                        sample_rate,
                        buffer_size,
                        ring_buffer_duration_ms,
                        _auto_reconnect,
                        high_priority,
                        dscp_strategy,
                        format,
                        chunk_size,
                        is_loopback,
                        is_server,
                        enable_adaptive_buffer,
                        min_buffer_ms,
                        max_buffer_ms,
                        app_handle,
                    )) = &_current_params
                    {
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
                            format.clone(),
                            chunk_size.clone(),
                            is_loopback.clone(),
                            is_server.clone(),
                            enable_adaptive_buffer.clone(),
                            min_buffer_ms.clone(),
                            max_buffer_ms.clone(),
                            app_handle.clone(),
                        ) {
                            Ok((stream, stats)) => {
                                if let Err(e) = stream.play() {
                                    emit_log(app_handle, "error", format!("Failed to play stream (reconnect): {}", e));
                                } else {
                                    _current_stream_handle = Some((stream, stats));
                                    emit_log(
                                        app_handle,
                                        "success",
                                        "Reconnected successfully".to_string(),
                                    );
                                }
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
        // Only log if it's NOT "Socket is not connected" (os error 57)
        if e.kind() != std::io::ErrorKind::NotConnected {
            emit_log(
                app_handle,
                "debug",
                format!(
                    "TCP shutdown {} ({}): socket may already be closed",
                    context, e
                ),
            );
        }
    } else {
        emit_log(
            app_handle,
            "debug",
            format!("TCP connection closed gracefully ({})", context),
        );
    }
}

fn start_audio_stream(
    device_name: String,
    // protocol removed
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
    enable_adaptive_buffer: bool,
    min_buffer_ms: u32,
    max_buffer_ms: u32,
    app_handle: AppHandle,
) -> Result<(cpal::Stream, StreamStats), String> {
    // Shadow sample_rate to allow override with native device rate
    let mut sample_rate = sample_rate;

    emit_log(
        &app_handle,
        "debug",
        format!(
            "Init stream: Mode={} Protocol=TCP, Device='{}', Rate={}, Buf={}, RingMs={}, Priority={}, DSCP={}, Format={}, Chunk={}, Loopback={}, AdaptiveBuf={} ({}ms-{}ms)",
            if is_server { "SERVER" } else { "CLIENT" }, device_name, sample_rate, buffer_size, ring_buffer_duration_ms, high_priority, dscp_strategy, format, chunk_size, is_loopback, enable_adaptive_buffer, min_buffer_ms, max_buffer_ms
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

    // 0.5. Query Device Default Config (to prevent sample rate mismatch/chipmunk effect)
    let default_config = if is_loopback {
         device.default_output_config()
    } else {
         device.default_input_config()
    };

    if let Ok(config) = default_config {
        let native_rate = config.sample_rate().0;
        if native_rate != sample_rate {
             emit_log(
                 &app_handle,
                 "warn",
                 format!("Overriding requested Sample Rate {} -> {} (Device Native)", sample_rate, native_rate)
             );
             sample_rate = native_rate;
        }
    } else {
         emit_log(&app_handle, "warn", "Failed to query device default config. Using requested rate.".to_string());
    }

    // 1. Setup Ring Buffer
    let adjusted_ring_buffer_duration_ms = if is_loopback {
        8000.max(ring_buffer_duration_ms)
    } else {
        5000.max(ring_buffer_duration_ms)
    };

    let ring_buffer_size =
        (sample_rate as usize) * 2 * (adjusted_ring_buffer_duration_ms as usize) / 1000;

    let adj_ms_u32 = adjusted_ring_buffer_duration_ms as u32;
    let buffer_size_mb = (ring_buffer_size * 2) as f32 / (1024.0 * 1024.0);

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

    let bytes_sent_clone = bytes_sent.clone();
    let is_running_clone = is_running.clone();
    let app_handle_net = app_handle.clone();
    let ip_clone = ip.clone();
    // protocol_clone removed
    let format_clone = format.clone();
    let sample_rate_clone = sample_rate;
    let is_server_clone = is_server;

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
        } else {
            if high_priority {
                emit_log(
                    &app_handle_net,
                    "info",
                    "Network thread priority set to Max".to_string(),
                );
            }
        }

        // Bind listener if in server mode (TCP only)
        let listener = if is_server_clone {
            match std::net::TcpListener::bind(format!("0.0.0.0:{}", port)) {
                Ok(l) => {
                    let _ = l.set_nonblocking(true);
                    emit_log(
                        &app_handle_net,
                        "success",
                        format!("TCP Server listening on 0.0.0.0:{}", port),
                    );
                    Some(l)
                }
                Err(e) => {
                    emit_log(
                        &app_handle_net,
                        "error",
                        format!("Failed to bind TCP server port: {}", e),
                    );
                    None
                }
            }
        } else {
            None
        };

        // UDP Server logic removed


        let mut sequence: u32 = 0;
        let mut temp_buffer = vec![0.0f32; chunk_size as usize];
        let start_time = Instant::now();
        let mut last_stats_emit = Instant::now();

        // Quality tracking variables
        let mut latency_samples: Vec<f32> = Vec::with_capacity(100);
        let mut jitter_avg: f32 = 0.0;
        let mut _last_write_time: Option<Instant> = None;
        let mut last_quality_emit = Instant::now();

        // MP3 Encoder Removed
        // let mut mp3_encoder: Option<mp3lame_encoder::Encoder> = ...


        // Initialize MP3 buffer if needed
        // MP3 Buffer Removed
        // let mp3_buffer_size = (chunk_size as usize * 2) + 7200;
        // let mut mp3_out_buffer = vec![0u8; mp3_buffer_size];

        // Adaptive buffer tracking
        let mut current_buffer_ms = adj_ms_u32;
        let mut last_buffer_check = Instant::now();
        const BUFFER_CHECK_INTERVAL_SECS: u64 = 10;

        let (adaptive_min_ms, adaptive_max_ms) = if is_loopback {
            (4000.max(min_buffer_ms), 12000.min(max_buffer_ms))
        } else {
            (2000.max(min_buffer_ms), 6000.min(max_buffer_ms))
        };

        let mut retry_delay = Duration::from_secs(2);
        const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);

        fn add_jitter(base: Duration) -> Duration {
            use std::time::SystemTime;
            let jitter_ms = (SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
                % 1000) as i64
                - 500;
            let ms = base.as_millis() as i64 + jitter_ms;
            Duration::from_millis(ms.max(2000) as u64)
        }

        let mut current_stream: Option<StreamSocket> = None;
        let mut disconnect_time: Option<Instant> = None;
        const DISCONNECT_FLUSH_THRESHOLD: Duration = Duration::from_secs(3);

        emit_log(
            &app_handle_net,
            "info",
            "Network thread started".to_string(),
        );

        let mut last_heartbeat = Instant::now();
        const HEARTBEAT_INTERVAL_SECS: u64 = 30;

        let mut prefilled = false;
        let prefill_samples = sample_rate as usize / 5; // 200ms prefill
        emit_log(
            &app_handle_net,
            "info",
            format!(
                "Buffering... waiting for {} samples (200ms)",
                prefill_samples
            ),
        );

        let tick_duration =
            Duration::from_micros((chunk_size as u64 * 1_000_000) / sample_rate as u64);

        let mut use_chunked = false;
        let mut wav_header_sent = false;
        let mut next_tick = Instant::now();
        let mut prefill_debug_timer = Instant::now();

        while is_running_clone.load(Ordering::Relaxed) {
            // 1. Strict Pacing
            let now = Instant::now();
            let current_buffered = cons.len();
            let high_water_mark = prefill_samples + (sample_rate as usize / 10);

            if current_buffered > high_water_mark && current_stream.is_some() {
                next_tick = now + tick_duration;
            } else if current_stream.is_none() && current_buffered > 0 {
                // Track how long we've been disconnected
                if disconnect_time.is_none() {
                    disconnect_time = Some(Instant::now());
                }

                let disconnected_for = disconnect_time.unwrap().elapsed();

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
                    // Short outage: drain gradually (existing behavior)
                    let drain_amount = (sample_rate as usize / 10).min(current_buffered);
                    let mut drain_buffer = vec![0.0f32; drain_amount];
                    let _ = cons.pop_slice(&mut drain_buffer);
                }

                if now < next_tick {
                    thread::sleep(next_tick - now);
                }
                next_tick = Instant::now() + tick_duration;
            } else {
                if now < next_tick {
                    thread::sleep(next_tick - now);
                    next_tick += tick_duration;
                } else {
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

            // 2. Connection Management
            if current_stream.is_none() {
                if is_server_clone {
                    // SERVER MODE
                    if let Some(l) = &listener {
                        // TCP Server
                        match l.accept() {
                            Ok((mut stream, addr)) => {
                                let _ = stream.set_nodelay(true);
                                
                                // Peek for HTTP
                                let mut buf = [0u8; 4];
                                let mut is_http = false;
                                let mut request_format = format_clone.clone(); // Default to configured

                                // Temporarily set blocking for a short peek? 
                                // Actually, since we just accepted, we can try to peek.
                                // If it blocks, it means client hasn't sent anything yet. 
                                // But browsers send immediately. 
                                // We can use a short timeout.
                                
                                // Note: set_read_timeout might not work for peek on all platforms/implementations but usually does on sockets
                                stream.set_read_timeout(Some(Duration::from_millis(1500))).ok();
                                
                                if let Ok(n) = stream.peek(&mut buf) {
                                    if n >= 4 && &buf[0..4] == b"GET " {
                                        is_http = true;
                                    }
                                }

                                if is_http {
                                    // Read Request Line
                                    let mut headers_buf = [0u8; 1024];
                                    if let Ok(n) = stream.read(&mut headers_buf) {
                                        let request = String::from_utf8_lossy(&headers_buf[0..n]);
                                        // Parse first line: GET /stream.wav HTTP/1.1
                                        if let Some(_line) = request.lines().next() {
                                            request_format = "wav".to_string();
                                        }
                                        
                                        // Send HTTP Headers with Chunked Encoding (Always WAV)
                                        let content_type = "audio/wav";
                                        let response = format!(
                                            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nTransfer-Encoding: chunked\r\nConnection: keep-alive\r\nCache-Control: no-cache, no-store, must-revalidate\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
                                            content_type
                                        );
                                        if let Err(e) = stream.write_all(response.as_bytes()) {
                                             emit_log(&app_handle_net, "error", format!("Failed to send HTTP headers: {}", e));
                                        } else {
                                             emit_log(&app_handle_net, "info", "HTTP Client detected. Serving audio/wav".to_string());
                                        }
                                    }
                                }

                                stream.set_read_timeout(None).ok(); // clear timeout
                                // KEEP BLOCKING MODE for stream to avoid partial writes/broken pipes
                                // let _ = stream.set_nonblocking(true);
                                
                                emit_log(
                                    &app_handle_net,
                                    "success",
                                    format!("Client connected from {} (Format: {})", addr, request_format),
                                );
                                
                                use_chunked = is_http;
                                wav_header_sent = false;
                                current_stream = Some(StreamSocket::Tcp(stream));
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
                    // CLIENT MODE (TCP Only)
                    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
                        .unwrap_or_else(|_| panic!("Failed to create socket"));
                    let _ = socket.set_send_buffer_size(32 * 1024);
                    let _ = socket.set_nodelay(true);
                    let keepalive = TcpKeepalive::new()
                        .with_time(Duration::from_secs(10))
                        .with_interval(Duration::from_secs(1));
                    let _ = socket.set_tcp_keepalive(&keepalive);

                    let tos_val = match dscp_strategy.as_str() {
                        "EF" => 0xB8,
                        "CS5" => 0xA0,
                        "LowDelay" => 0x10,
                        _ => 0x00,
                    };

                    if tos_val > 0 {
                        if let Err(e) = socket.set_tos(tos_val) {
                            emit_log(
                                &app_handle_net,
                                "warning",
                                format!("Failed to set QoS/TOS: {}", e),
                            );
                        }
                    }

                    // For client mode, ip_clone is the target IP
                    let addr_str = format!("{}:{}", ip_clone, port);
                    if let Ok(addr) = addr_str.parse::<SocketAddr>() {
                        match socket.connect_timeout(&addr.into(), Duration::from_secs(2)) {
                            Ok(_) => {
                                let stream: TcpStream = socket.into();
                                // KEEP BLOCKING MODE
                                // let _ = stream.set_nonblocking(true);
                                emit_log(
                                    &app_handle_net,
                                    "success",
                                    format!("Connected to {}", addr_str),
                                );
                                current_stream = Some(StreamSocket::Tcp(stream));
                                disconnect_time = None;
                                retry_delay = Duration::from_secs(2);
                                last_heartbeat = Instant::now();
                            }
                            Err(e) => {
                                emit_log(
                                    &app_handle_net,
                                    "error",
                                    format!("Connection failed: {}", e),
                                );
                                thread::sleep(add_jitter(retry_delay));
                                retry_delay = (retry_delay * 2).min(MAX_RETRY_DELAY);
                            }
                        }
                    } else {
                        emit_log(
                            &app_handle_net,
                            "error",
                            format!("Invalid address: {}", addr_str),
                        );
                        thread::sleep(Duration::from_secs(5));
                    }
                }
            }

            // 3. Send Data
            if let Some(mut stream_socket) = current_stream.take() {
                // Send WAV header if needed
                // Server Mode:
                // - "wav": Header + PCM (Browsers/VLC)
                // - "pcm": Raw PCM (Snapserver)
                if is_server_clone && format_clone == "wav" && !wav_header_sent {
                     let StreamSocket::Tcp(ref mut stream) = stream_socket;
                     {
                         // Send header with 0 length (handled by helper) for unknown/stream
                         let header = create_wav_header(sample_rate_clone, 1, 16); 
                         
                         if use_chunked {
                             let mut writer = ChunkedWriter::new(stream);
                             if let Err(e) = writer.write_all(&header) {
                                  let level = if e.kind() == std::io::ErrorKind::BrokenPipe { "info" } else { "error" };
                                  emit_log(&app_handle_net, level, format!("Failed to send WAV header (chunked): {}", e));
                             } else {
                                 wav_header_sent = true;
                             }
                         } else {
                             if let Err(e) = stream.write_all(&header) {
                                  let level = if e.kind() == std::io::ErrorKind::BrokenPipe { "info" } else { "error" };
                                  emit_log(&app_handle_net, level, format!("Failed to send WAV header: {}", e));
                             } else {
                                  wav_header_sent = true;
                             }
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
                    let data_len = (count * 2) as u32;
                    // Convert Float to PCM i16
                    let mut payload = Vec::with_capacity(data_len as usize);
                    for i in 0..count {
                        let sample = temp_buffer[i].clamp(-1.0, 1.0);
                        let sample_i16 = (sample * 32767.0) as i16;
                        payload.extend_from_slice(&sample_i16.to_le_bytes());
                    }

                    // Jitter Calc
                    let send_time = Instant::now();
                    let target = next_tick - tick_duration;
                    let deviation_ms = if send_time > target {
                        send_time.duration_since(target).as_secs_f32() * 1000.0
                    } else {
                        0.0
                    };

                    if jitter_avg == 0.0 {
                        jitter_avg = deviation_ms;
                    } else {
                        jitter_avg = 0.9 * jitter_avg + 0.1 * deviation_ms;
                    }

                    let write_start = Instant::now();
                    let mut write_success = false;
                    let mut would_block = false;

                    match stream_socket {
                        StreamSocket::Tcp(ref mut stream) => {
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

                    }

                    if write_success {
                        let write_duration_ms = write_start.elapsed().as_secs_f32() * 1000.0;
                        if latency_samples.len() >= 100 {
                            latency_samples.remove(0);
                        }
                        latency_samples.push(write_duration_ms);

                        let _ =
                            bytes_sent_clone.fetch_add(payload.len() as u64, Ordering::Relaxed);
                        sequence = sequence.wrapping_add(1);
                        _last_write_time = Some(Instant::now());

                        current_stream = Some(stream_socket);
                    } else if would_block {
                        current_stream = Some(stream_socket);
                    } else {
                         // Error handling
                         let StreamSocket::Tcp(s) = stream_socket;
                         {
                             close_tcp_stream(s, "write error", &app_handle_net);
                         }
                         current_stream = None; 
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

            // Quality Logic
            if last_quality_emit.elapsed() >= Duration::from_secs(4) {
                let avg_latency = if !latency_samples.is_empty() {
                    latency_samples.iter().sum::<f32>() / latency_samples.len() as f32
                } else {
                    0.0
                };
                let occupied = cons.len();
                let capacity = cons.capacity();
                let buffer_health = 1.0 - (occupied as f32 / capacity as f32);
                let jitter_penalty = ((jitter_avg / 20.0).min(1.0) * 50.0) as u8;
                let buffer_pct = occupied as f32 / capacity as f32;
                let buffer_penalty = if buffer_pct > 0.5 {
                    ((buffer_pct - 0.5) * 2.0 * 30.0) as u8
                } else {
                    0
                };
                let score = 100u8
                    .saturating_sub(jitter_penalty)
                    .saturating_sub(buffer_penalty);

                let _ = app_handle_net.emit(
                    "quality-event",
                    QualityEvent {
                        score,
                        jitter: jitter_avg,
                        avg_latency,
                        buffer_health,
                        error_count: 0,
                    },
                );
                last_quality_emit = Instant::now();

                // Adaptive Buffer
                if enable_adaptive_buffer
                    && last_buffer_check.elapsed()
                        >= Duration::from_secs(BUFFER_CHECK_INTERVAL_SECS)
                {
                    last_buffer_check = Instant::now();
                    if buffer_health < 0.2 {
                        if current_buffer_ms > adaptive_min_ms {
                            let new_size =
                                current_buffer_ms.saturating_sub(500).max(adaptive_min_ms);
                            if new_size < current_buffer_ms {
                                current_buffer_ms = new_size;
                                emit_log(
                                    &app_handle_net,
                                    "info",
                                    format!(
                                        "Adaptive Buffer: High latency. Reducing to {}ms",
                                        current_buffer_ms
                                    ),
                                );
                                let _ = app_handle_net.emit(
                                    "buffer-resize",
                                    BufferResizeEvent {
                                        new_size_ms: current_buffer_ms,
                                        reason: "High Latency".to_string(),
                                    },
                                );
                            }
                        }
                    } else if buffer_health > 0.8 {
                        if current_buffer_ms < adaptive_max_ms {
                            let new_size =
                                current_buffer_ms.saturating_add(500).min(adaptive_max_ms);
                            if new_size > current_buffer_ms {
                                current_buffer_ms = new_size;
                                emit_log(
                                    &app_handle_net,
                                    "info",
                                    format!(
                                        "Adaptive Buffer: Underflow risk. Increasing to {}ms",
                                        current_buffer_ms
                                    ),
                                );
                                let _ = app_handle_net.emit(
                                    "buffer-resize",
                                    BufferResizeEvent {
                                        new_size_ms: current_buffer_ms,
                                        reason: "Underflow Risk".to_string(),
                                    },
                                );
                            }
                        }
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

    // 3. Setup Audio Stream (Producer)
    let supported_configs: Vec<_> = device.supported_input_configs().map_err(|e| e.to_string())?.collect();
    let mut selected_format = cpal::SampleFormat::F32;
    let mut best_config_range = None;

    for channels in [1, 2] {
        for format in [cpal::SampleFormat::F32, cpal::SampleFormat::I16, cpal::SampleFormat::U16] {
            for range in &supported_configs {
                if range.sample_format() == format && range.channels() == channels {
                    if range.min_sample_rate().0 <= sample_rate && range.max_sample_rate().0 >= sample_rate {
                        best_config_range = Some(range.clone());
                        selected_format = format;
                        break;
                    }
                }
            }
            if best_config_range.is_some() { break; }
        }
        if best_config_range.is_some() { break; }
    }

    if best_config_range.is_none() && !supported_configs.is_empty() {
        let fallback = supported_configs.first().unwrap();
        selected_format = fallback.sample_format();
        best_config_range = Some(fallback.clone());
    }

    let config_range = best_config_range.ok_or_else(|| "No supported audio config found".to_string())?;
    let channels = config_range.channels();

    emit_log(
        &app_handle,
        "info",
        format!("Detected Input Format: {:?} ({} channels)", selected_format, channels),
    );

    let stream_config = cpal::StreamConfig {
        channels,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let err_fn = move |err| {
        error!("an error occurred on stream: {}", err);
    };

    // Shared producer (mutex wrapped for multiple closures)
    let prod = Arc::new(Mutex::new(prod));

    let audio_stream = match selected_format {
        cpal::SampleFormat::F32 => {
            let prod_clone = prod.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[f32], _: &_| {
                    if let Ok(mut guard) = prod_clone.lock() {
                        let _ = guard.push_slice(data);
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let prod_clone = prod.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _: &_| {
                    let mut converted = Vec::with_capacity(data.len());
                    for &sample in data {
                        converted.push(sample as f32 / 32768.0);
                    }
                    if let Ok(mut guard) = prod_clone.lock() {
                        let _ = guard.push_slice(&converted);
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let prod_clone = prod.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[u16], _: &_| {
                    let mut converted = Vec::with_capacity(data.len());
                    for &sample in data {
                        converted.push((sample as f32 - 32768.0) / 32768.0);
                    }
                    if let Ok(mut guard) = prod_clone.lock() {
                        let _ = guard.push_slice(&converted);
                    }
                },
                err_fn,
                None,
            )
        }
        _ => return Err(format!("Unsupported format: {:?}", selected_format))
    }.map_err(|e| e.to_string())?;

    Ok((
        audio_stream,
        StreamStats {
            bytes_sent,
            start_time: Instant::now(),
            is_running,
        },
    ))
}
