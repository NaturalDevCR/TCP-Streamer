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

// Health Event
#[derive(Clone, Serialize)]
struct HealthEvent {
    buffer_usage: f32,    // 0.0 to 1.0
    network_latency: u32, // ms (estimated based on write time)
    dropped_packets: u64,
}

#[derive(Clone, Serialize)]
struct StatsEvent {
    uptime_seconds: u64,
    bytes_sent: u64,
    bitrate_kbps: f64,
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
        silence_threshold: f32,
        silence_timeout_seconds: u64,
        is_loopback: bool,
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
            // (device_name, ip, port, sample_rate, buffer_size, ring_buffer_duration_ms, auto_reconnect, high_priority, dscp_strategy, chunk_size, silence_threshold, silence_timeout_seconds, is_loopback, app_handle)
            let mut _current_params: Option<(String, String, u16, u32, u32, u32, bool, bool, String, u32, f32, u64, bool, AppHandle)> = None;

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
                        silence_threshold,
                        silence_timeout_seconds,
                        is_loopback,
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
                            silence_threshold,
                            silence_timeout_seconds,
                            is_loopback,
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
                            silence_threshold,
                            silence_timeout_seconds,
                            is_loopback,
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
                        silence_threshold,
                        silence_timeout_seconds,
                        is_loopback,
                        app_handle,
                    )) = &_current_params
                    {
                        // Try to reconnect
                        // In a real app, we'd want a delay here to avoid tight loops
                        thread::sleep(Duration::from_secs(2));
                        
                        emit_log(
                            app_handle,
                            "info",
                            "Attempting to reconnect...".to_string(),
                        );

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
                            silence_threshold.clone(),
                            silence_timeout_seconds.clone(),
                            is_loopback.clone(),
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
    silence_threshold: f32,
    silence_timeout_seconds: u64,
    is_loopback: bool,
    app_handle: AppHandle,
) -> Result<(cpal::Stream, StreamStats), String> {
    emit_log(
        &app_handle,
        "debug",
        format!(
            "Initializing audio stream: Device='{}', Rate={}, Buf={}, RingBufMs={}, Priority={}, DSCP={}, Chunk={}, SilenceThreshold={}, SilenceTimeout={}s, Loopback={}",
            device_name, sample_rate, buffer_size, ring_buffer_duration_ms, high_priority, dscp_strategy, chunk_size, silence_threshold, silence_timeout_seconds, is_loopback
        ),
    );

    let host = cpal::default_host();
    
    // Device selection logic
    let device = if is_loopback {
        // Loopback mode: Search in OUTPUT devices
        // Remove "[Loopback] " prefix if present for matching
        let clean_name = device_name.replace("[Loopback] ", "");
        let mut found_device = None;
        
        if let Ok(devices) = host.output_devices() {
            for dev in devices {
                if let Ok(name) = dev.name() {
                    if name == clean_name {
                        found_device = Some(dev);
                        break;
                    }
                }
            }
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

    let config = cpal::StreamConfig {
        channels: 2,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Fixed(buffer_size),
    };

    // 1. Setup Ring Buffer
    // Size calculated from duration (ms)
    // ring_buffer_duration_ms default should be around 4000ms for stability
    let ring_buffer_size = (sample_rate as usize) * 2 * (ring_buffer_duration_ms as usize) / 1000;
    emit_log(
        &app_handle,
        "trace",
        format!("Allocating ring buffer of size: {}", ring_buffer_size),
    );
    let rb = HeapRb::<i16>::new(ring_buffer_size);
    let (mut prod, mut cons) = rb.split();

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
        let mut temp_buffer = vec![0i16; chunk_size as usize];
        let mut dropped_packets: u64 = 0;
        let start_time = Instant::now();
        let mut last_stats_emit = Instant::now();
        
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

        while is_running_clone.load(Ordering::Relaxed) {
            // 1. Handle Reconnection if needed
            if current_stream.is_none() {
                emit_log(
                    &app_handle_net,
                    "warning",
                    format!("Reconnecting to {} (retry in {}s)...", 
                        server_addr, retry_delay.as_secs()),
                );
                
                // Wait before attempting reconnection (exponential backoff)
                thread::sleep(retry_delay);

                // Advanced Socket Setup using socket2
                let connect_result = (|| -> Result<TcpStream, Box<dyn std::error::Error>> {
                    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;

                    // 1. Set Send Buffer Size (1MB)
                    // This allows the kernel to buffer more data if the network is momentarily slow,
                    // preventing the application from blocking on write immediately.
                    socket.set_send_buffer_size(1024 * 1024)?;
                    emit_log(
                        &app_handle_net,
                        "trace",
                        "Socket send buffer set to 1MB".to_string(),
                    );

                    // 2. Set Keepalive
                    // Detect dead connections faster
                    let keepalive = TcpKeepalive::new()
                        .with_time(Duration::from_secs(10))
                        .with_interval(Duration::from_secs(1));
                    socket.set_tcp_keepalive(&keepalive)?;
                    emit_log(&app_handle_net, "trace", "Socket keepalive set".to_string());

                    // 3. Set Type of Service (TOS) / DSCP
                    let tos_value = match dscp_strategy.as_str() {
                        "voip" => 0xB8,       // EF
                        "lowdelay" => 0x10,   // IPTOS_LOWDELAY
                        "throughput" => 0x08, // IPTOS_THROUGHPUT
                        "besteffort" => 0x00,
                        _ => 0xB8, // Default to EF
                    };

                    if let Err(e) = socket.set_tos(tos_value) {
                        emit_log(
                            &app_handle_net,
                            "warning",
                            format!("Failed to set DSCP {:#x}: {}. Using default.", tos_value, e),
                        );
                    } else {
                        emit_log(
                            &app_handle_net,
                            "debug",
                            format!("DSCP set to {:#x} ({})", tos_value, dscp_strategy),
                        );
                    }

                    // 4. Connect
                    let addr: SocketAddr = server_addr.parse()?;
                    emit_log(
                        &app_handle_net,
                        "debug",
                        format!("Connecting to address: {:?}", addr),
                    );
                    socket.connect(&addr.into())?;

                    // 5. Convert to std::net::TcpStream
                    let stream: TcpStream = socket.into();
                    stream.set_nodelay(true)?; // Ensure Nagle's algo is off
                    emit_log(
                        &app_handle_net,
                        "trace",
                        "Nagle's algorithm disabled (TCP_NODELAY)".to_string(),
                    );

                    Ok(stream)
                })();

                match connect_result {
                    Ok(s) => {
                        // Reset retry delay on successful connection
                        retry_delay = Duration::from_secs(1);
                        
                        // We do NOT send headers anymore because Snapserver TCP source expects RAW PCM.
                        // Sending text headers causes byte misalignment (byte shift) if the length
                        // is not a multiple of 4 (16-bit stereo), resulting in severe audio distortion.
                        // The stream format must be configured on the Snapserver side (e.g., sampleformat=48000:16:2).
                        
                        current_stream = Some(s);
                        emit_log(
                            &app_handle_net,
                            "success",
                            "Connected successfully! (Raw PCM)".to_string(),
                        );
                    }
                    Err(e) => {
                        emit_log(
                            &app_handle_net,
                            "error",
                            format!("Reconnection failed: {}", e),
                        );
                        // Exponential backoff: double the delay for next attempt
                        retry_delay = std::cmp::min(retry_delay * 2, MAX_RETRY_DELAY);
                        continue; // Retry loop
                    }
                }
            }

            // Check buffer usage for health monitoring
            let occupied = cons.len();
            let capacity = cons.capacity();
            let usage = occupied as f32 / capacity as f32;

            if usage > 0.9 {
                dropped_packets += 1;
                if dropped_packets % 100 == 0 {
                    emit_log(
                        &app_handle_net,
                        "warning",
                        format!(
                            "High buffer usage: {:.1}%. Dropping packets.",
                            usage * 100.0
                        ),
                    );
                }
            }

            // Emit health event occasionally
            if sequence % 100 == 0 {
                let _ = app_handle_net.emit(
                    "health-event",
                    HealthEvent {
                        buffer_usage: usage,
                        network_latency: 0,
                        dropped_packets,
                    },
                );
            }

            // Read from Ring Buffer
            let count = cons.pop_slice(&mut temp_buffer);

            if count > 0 {
                let start_write = Instant::now();

                // Prepare Payload (Raw PCM)
                let data_len = (count * 2) as u32;
                let mut payload = Vec::with_capacity(data_len as usize);
                for i in 0..count {
                    payload.extend_from_slice(&temp_buffer[i].to_le_bytes());
                }

                // Send Payload
                if let Some(ref mut s) = current_stream {
                    if let Err(e) = s.write_all(&payload) {
                        emit_log(&app_handle_net, "error", format!("Write error: {}", e));
                        // Drop connection to trigger reconnect
                        current_stream = None;
                        continue;
                    }
                }

                let _write_time = start_write.elapsed().as_millis();
                // Update stats
                let _ = bytes_sent_clone.fetch_add(payload.len() as u64, Ordering::Relaxed);
                sequence = sequence.wrapping_add(1);
            } else {
                // Buffer empty, sleep briefly to avoid busy loop
                thread::sleep(Duration::from_millis(1));
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
        }
        emit_log(
            &app_handle_net,
            "info",
            "Network thread stopped".to_string(),
        );
    });

    // 3. Setup Audio Stream (Producer)
    let app_handle_err = app_handle.clone();
    let err_fn = move |err| {
        emit_log(&app_handle_err, "error", format!("Stream error: {}", err));
    };

    // RMS Silence Detection State
    // silence_threshold is now passed as a parameter
    let mut silence_start: Option<Instant> = None;
    let app_handle_audio = app_handle.clone();

    let audio_stream = device
        .build_input_stream(
            &config,
            move |data: &[i16], _: &_| {
                // Calculate RMS for silence detection
                let mut sum_squares = 0.0;
                for &sample in data {
                    sum_squares += (sample as f32) * (sample as f32);
                }
                let rms = (sum_squares / data.len() as f32).sqrt();

                if rms > silence_threshold {
                    // Audio detected
                    if let Some(start) = silence_start {
                        let duration = start.elapsed();
                        if duration.as_secs() > 1 {
                            emit_log(
                                &app_handle_audio,
                                "info",
                                format!(
                                    "Audio resumed (RMS: {:.1}) after {:.1}s silence [Threshold: {:.1}]",
                                    rms, duration.as_secs_f32(), silence_threshold
                                ),
                            );
                        }
                        silence_start = None;
                    }
                    // Always push actual audio data
                    let pushed = prod.push_slice(data);
                    if pushed < data.len() {
                        // Buffer overflow
                    }
                } else {
                    // Silence detected
                    if silence_start.is_none() {
                        silence_start = Some(Instant::now());
                        emit_log(
                            &app_handle_audio, 
                            "debug", 
                            format!(
                                "Silence detected (RMS: {:.1} < Threshold: {:.1})",
                                rms, silence_threshold
                            )
                        );
                    }
                    
                    let silence_duration = silence_start.as_ref().unwrap().elapsed();
                    
                    if silence_timeout_seconds == 0 || silence_duration.as_secs() < silence_timeout_seconds {
                        // Within grace period or timeout disabled - keep transmitting
                        let pushed = prod.push_slice(data);
                        if pushed < data.len() {
                            // Buffer overflow
                        }
                    } else {
                        // Timeout exceeded - stop transmission to save bandwidth
                        if silence_duration.as_secs() == silence_timeout_seconds {
                            emit_log(
                                &app_handle_audio,
                                "info",
                                format!("Silence timeout ({}s) - stopping transmission", silence_timeout_seconds)
                            );
                        }
                        // Don't push anything - saves bandwidth
                    }
                }
            },
            err_fn,
            None,
        )
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
pub fn get_input_devices(_include_loopback: bool) -> Result<Vec<String>, String> {
    let mut all_devices = Vec::new();

    // Try all available hosts
    for host_id in cpal::available_hosts() {
        let host = cpal::host_from_id(host_id).map_err(|e| e.to_string())?;
        debug!("Scanning host: {:?}", host_id);

        // 1. Standard Input Devices
        if let Ok(devices) = host.input_devices() {
            for device in devices {
                if let Ok(name) = device.name() {
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
    silence_threshold: f32,
    silence_timeout_seconds: u64,
    is_loopback: bool,
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
        silence_threshold,
        silence_timeout_seconds,
        is_loopback,
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
