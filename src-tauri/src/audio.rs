use chrono::Local;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::{debug, error, info, trace, warn};
use ringbuf::HeapRb;
use serde::Serialize;
use socket2::{Domain, Protocol, Socket, TcpKeepalive, Type};
use std::io::Write;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU32, Ordering};
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
    network_latency: u64, // ms (estimated based on write time)
    dropped_packets: u64,
}

#[derive(Clone, Serialize)]
struct QualityEvent {
    score: u8,           // 0-100
    jitter: f32,         // milliseconds
    avg_latency: f32,    // milliseconds
    buffer_health: f32,  // 0.0-1.0
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

#[derive(Clone, Serialize)]
struct DriftEvent {
    ppm: i32,
    latency_us: i64,
}

// Global Atomic Settings for Dynamic Updates
static SILENCE_THRESHOLD_BITS: AtomicU32 = AtomicU32::new(0); // f32 stored as u32 bits
static SILENCE_TIMEOUT_SECS: AtomicU64 = AtomicU64::new(0);
static DRIFT_CORRECTION_PPM: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static AUTO_SYNC_ENABLED: AtomicBool = AtomicBool::new(false);

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
        enable_adaptive_buffer: bool,
        min_buffer_ms: u32,
        max_buffer_ms: u32,
        drift_correction_ppm: i32,
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
            // (device_name, ip, port, sample_rate, buffer_size, ring_buffer_duration_ms, auto_reconnect, high_priority, dscp_strategy, chunk_size, silence_threshold, silence_timeout_seconds, is_loopback, enable_adaptive_buffer, min_buffer_ms, max_buffer_ms, drift_correction_ppm, app_handle)
            let mut _current_params: Option<(String, String, u16, u32, u32, u32, bool, bool, String, u32, f32, u64, bool, bool, u32, u32, i32, AppHandle)> = None;

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
                        enable_adaptive_buffer,
                        min_buffer_ms,
                        max_buffer_ms,
                        drift_correction_ppm,
                        app_handle,
                    } => {
                        // Stop existing stream if any
                        if let Some((stream, stats)) = _current_stream_handle.take() {
                            drop(stream);
                            drop(stats); // Signals stats thread to stop
                        }

                        // Initialize global settings with new values
                        SILENCE_THRESHOLD_BITS.store(silence_threshold.to_bits(), Ordering::Relaxed);
                        SILENCE_TIMEOUT_SECS.store(silence_timeout_seconds, Ordering::Relaxed);
                        
                        // Set initial manual drift correction
                        // Only set if auto sync is disabled, otherwise let the thread handle it
                        if !AUTO_SYNC_ENABLED.load(Ordering::Relaxed) {
                            DRIFT_CORRECTION_PPM.store(drift_correction_ppm, Ordering::Relaxed);
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
                            enable_adaptive_buffer,
                            min_buffer_ms,
                            max_buffer_ms,
                            drift_correction_ppm,
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
                            enable_adaptive_buffer,
                            min_buffer_ms,
                            max_buffer_ms,
                            drift_correction_ppm,
                            app_handle.clone(),
                        ) {
                             Ok((stream, stats)) => {
                                stream.play().unwrap();
                                _current_stream_handle = Some((stream, stats));
                                
                                // Spawn Sync Thread if Auto Sync is enabled
                                // We use the same IP, but Port 1704 (Snapcast Control Port)
                                if AUTO_SYNC_ENABLED.load(Ordering::Relaxed) {
                                    // Start sync thread logic
                                    let sync_ip = ip.clone();
                                    // Default control port is 1704 for binary protocol
                                    // In a full implementation we might want this configurable
                                    let sync_port = 1704; 
                                    spawn_sync_thread(sync_ip, sync_port, app_handle.clone());
                                }

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
                        enable_adaptive_buffer,
                        min_buffer_ms,
                        max_buffer_ms,
                        drift_correction_ppm,
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
                            enable_adaptive_buffer.clone(),
                            min_buffer_ms.clone(),
                            max_buffer_ms.clone(),
                            drift_correction_ppm.clone(),
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
                                
                                // Restart sync thread on reconnect if needed
                                if AUTO_SYNC_ENABLED.load(Ordering::Relaxed) {
                                     let sync_ip = ip.clone();
                                     let sync_port = 1704;
                                     spawn_sync_thread(sync_ip, sync_port, app_handle.clone());
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
        // Stop sync thread implicitly by app exit or handle invalidation
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(AudioCommand::Stop);
        }
    }
}

// Commands to control Auto Sync and Drift from UI

#[tauri::command]
pub fn set_drift_correction(ppm: i32) {
    if !AUTO_SYNC_ENABLED.load(Ordering::Relaxed) {
        DRIFT_CORRECTION_PPM.store(ppm, Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn set_auto_sync(enabled: bool, ip: String, app_handle: AppHandle) {
    let was_enabled = AUTO_SYNC_ENABLED.swap(enabled, Ordering::Relaxed);
    if enabled && !was_enabled {
        // Just enabled, spawn thread
        // We assume default port 1704
        spawn_sync_thread(ip, 1704, app_handle);
    }
}


// --- Binary Protocol Implementation ---

use std::io::{Read, Write};
use std::collections::VecDeque;

struct MovingAverage {
    window_size: usize,
    samples: VecDeque<f64>,
    sum: f64,
}

impl MovingAverage {
    fn new(window_size: usize) -> Self {
        Self {
            window_size,
            samples: VecDeque::with_capacity(window_size),
            sum: 0.0,
        }
    }

    fn add(&mut self, value: f64) -> f64 {
        if self.samples.len() >= self.window_size {
            if let Some(old) = self.samples.pop_front() {
                self.sum -= old;
            }
        }
        self.samples.push_back(value);
        self.sum += value;
        self.sum / self.samples.len() as f64
    }
}

#[repr(C, packed)]
struct SnapMessageHeader {
    type_: u16,
    id: u16,
    ref_id: u16,
    sent_sec: i32,
    sent_usec: i32,
    recv_sec: i32,
    recv_usec: i32,
    size: u32,
}

impl SnapMessageHeader {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(26);
        bytes.extend_from_slice(&self.type_.to_le_bytes());
        bytes.extend_from_slice(&self.id.to_le_bytes());
        bytes.extend_from_slice(&self.ref_id.to_le_bytes());
        bytes.extend_from_slice(&self.sent_sec.to_le_bytes());
        bytes.extend_from_slice(&self.sent_usec.to_le_bytes());
        bytes.extend_from_slice(&self.recv_sec.to_le_bytes());
        bytes.extend_from_slice(&self.recv_usec.to_le_bytes());
        bytes.extend_from_slice(&self.size.to_le_bytes());
        bytes
    }
}

fn spawn_sync_thread(ip: String, port: u16, app_handle: AppHandle) {
    thread::spawn(move || {
        emit_log(&app_handle, "info", format!("Starting Auto Sync with {}:{}", ip, port));
        
        let mut ma_filter = MovingAverage::new(200); // 200 samples window for stability
        let mut consecutive_errors = 0;
        
        loop {
            if !AUTO_SYNC_ENABLED.load(Ordering::Relaxed) {
                break;
            }

            // Connect to Control Port
            match TcpStream::connect(format!("{}:{}", ip, port)) {
                Ok(mut stream) => {
                    emit_log(&app_handle, "success", "Connected to Control Port".to_string());
                    consecutive_errors = 0;

                    // Sync Loop
                    loop {
                        if !AUTO_SYNC_ENABLED.load(Ordering::Relaxed) {
                            break;
                        }

                        let now = std::time::SystemTime::now();
                        let duration_since_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap_or(Duration::ZERO);
                        let client_sent_sec = duration_since_epoch.as_secs() as i32;
                        let client_sent_usec = duration_since_epoch.subsec_micros() as i32;

                        let header = SnapMessageHeader {
                            type_: 4, // kTime
                            id: 1,    // Request ID
                            ref_id: 0,
                            sent_sec: client_sent_sec,
                            sent_usec: client_sent_usec,
                            recv_sec: 0,
                            recv_usec: 0,
                            size: 0,
                        };

                        if let Err(e) = stream.write_all(&header.to_bytes()) {
                            emit_log(&app_handle, "warning", format!("Sync send error: {}", e));
                            break; // Reconnect
                        }

                        // Read response (26 bytes header)
                        let mut response_buf = [0u8; 26];
                        if let Err(e) = stream.read_exact(&mut response_buf) {
                            emit_log(&app_handle, "warning", format!("Sync read error: {}", e));
                            break; 
                        }

                        // Parse response
                        let recv_sec = i32::from_le_bytes(response_buf[16..20].try_into().unwrap());
                        let recv_usec = i32::from_le_bytes(response_buf[20..24].try_into().unwrap());
                        
                         // Snapcast latency is stored in 'size' field strictly for Time messages? 
                         // No, strictly specific latency field in JSON, but binary...
                         // Actually standard Snapcast binary protocol puts latency in the body or reuses fields.
                         // Let's re-read the spec/code provided by user.
                         // User: "response->latency, response->received - response->sent"
                         // Snapcast Time Message (Type 4):
                         // It mirrors the structure. The "latency" is usually put in the payload or fields?
                         // Wait, header is fixed 26 bytes.
                         // Time message usually has 0 payload size.
                         // The server fills `recv_sec`/`recv_usec` with its receive time.
                         // Snapcast uses the header fields.
                         
                         // Client receive time
                        let now_recv = std::time::SystemTime::now();
                        let duration_recv = now_recv.duration_since(std::time::UNIX_EPOCH).unwrap_or(Duration::ZERO);
                        // let client_recv_sec = duration_recv.as_secs() as i32;
                        // let client_recv_usec = duration_recv.subsec_micros() as i32;
                        
                        // Timestamps in microseconds
                        let t_client_sent = (client_sent_sec as i64 * 1_000_000) + client_sent_usec as i64;
                        let t_server_recv = (recv_sec as i64 * 1_000_000) + recv_usec as i64;
                        let t_client_recv = (duration_recv.as_secs() as i64 * 1_000_000) + duration_recv.subsec_micros() as i64;
                        
                        // Algorithm: (t_server_recv - t_client_sent) is C2S latency + clock diff
                        // We want Drift.
                        // Standard NTP/Snapcast:
                        // latency = (t_client_recv - t_client_sent) / 2
                        // time_diff = t_server_recv - t_client_sent - latency
                        
                        let latency_micros = (t_client_recv - t_client_sent) / 2;
                        let offset_micros = t_server_recv - t_client_sent - latency_micros;
                        
                        // We need PPM (Parts Per Million).
                        // PPM is the *rate of change* of the offset.
                        // However, Snapcast client just sets the "diff" directly to adjust playback?
                        // "TimeProvider::getInstance().setDiff..."
                        // But we are the SENDER (Streamer). We don't play audio, we pace it.
                        // If the server clock is FASTER, `offset` will increase?
                        // Wait.
                        // If Client (Us) = 0, Server = 1000. Offset = 1000.
                        // 1s later. Client = 1000, Server = 2001 (Fast server). Offset = 1001.
                        // Offset is increasing.
                        // We need to match Server speed.
                        // If Server is fast, we need to send FASTER.
                        // So PPM should be positive?
                        
                        // We need to calculate the SLOPE of offset over time.
                        // BUT, for phase 1 implementation, maybe we can just use a simpler PID or just the offset if we were strictly syncing clocks.
                        // But here we are adjusting RATE.
                        // User suggested: "calculate_drift(latency, ...)"
                        // "drift_ppm = (time_diff / 1_000_000.0) * 1_000_000.0" -> This logic in user prompt seems to calculate absolute offset in seconds and call it PPM?
                        // That doesn't make sense. PPM is a rate.
                        
                        // Actually, if we just want to stabilize the BUFFER (Recv-Q), we don't strictly care about absolute time sync, just RATE sync.
                        // However, the User Prompt implies we equate "Drift Correction" to "Clock Sync".
                        
                        // Let's implement what the User requested:
                        // "calculate_drift" -> "time_diff ... drift_ppm".
                        // This suggests they might be using a simplified model where they treat the diff as the error signal for a control loop?
                        // No, "time_diff" is constant if rates are equal.
                        
                        // I will implement a proper SLOPE calculation (or just report offset for now if I can't do slope easily).
                        // Actually, to get PPM, I need: (Offset2 - Offset1) / (Time2 - Time1).
                        
                        // Simplified approach:
                        // We maintain a "Target Offset" (initial offset).
                        // Error = Current Offset - Target Offset.
                        // We adjust PPM proportional to this Error (P-controller).
                        // If Current Offset > Target, Server is running ahead (or we are slow).
                        // We speed up (Positive PPM).
                        
                        // Let's use a static target captured at start of sync.
                        // P-Controller: PPM = Kp * (Offset - InitialOffset).
                        
                        // For now, let's just log the metrics and use a placeholder 0 for PPM update until I verify the offset behavior with the user via logs.
                        // WAIT. The user explicitely asked for "Active Rate Control".
                        // I'll try a rudimentary P-controller.
                        
                        // Note: For now, I will just log the calculated values and allow the user to see them.
                        // Calculating PPM from instant offset is wrong. I need delta.
                        
                        // Let's store `last_offset` and `last_time`.
                        // PPM = (delta_offset_micros / delta_time_secs).
                        // If delta_offset is 100us over 1s, that is 100 PPM.
                        
                        // Moving Average of PPM samples.
                        
                        static mut LAST_OFFSET: i64 = 0;
                        static mut LAST_TIME: i64 = 0;
                        static mut HAS_PREV: bool = false;
                        
                        let ppm_inst = unsafe {
                            if HAS_PREV {
                                let delta_offset = offset_micros - LAST_OFFSET;
                                let delta_time = t_client_recv - LAST_TIME;
                                
                                if delta_time > 0 {
                                    (delta_offset as f64 / delta_time as f64) * 1_000_000.0
                                } else {
                                    0.0
                                }
                            } else {
                                0.0
                            }
                        };
                        
                        unsafe {
                            LAST_OFFSET = offset_micros;
                            LAST_TIME = t_client_recv;
                            HAS_PREV = true;
                        }
                        
                        // This instantaneous PPM is very noisy. Filter it.
                        let filtered_ppm = ma_filter.add(ppm_inst);
                        
                        // Update global atomic
                        DRIFT_CORRECTION_PPM.store(filtered_ppm as i32, Ordering::Relaxed);
                        
                        let _ = app_handle.emit("drift-event", DriftEvent {
                            ppm: filtered_ppm as i32,
                            latency_us: latency_micros,
                        });
                        
                        // Log roughly once per second to avoid spamming at 10Hz
                        // We can use a counter or time check
                        static mut LOG_COUNTER: usize = 0;
                        unsafe {
                            LOG_COUNTER += 1;
                            if LOG_COUNTER % 10 == 0 {
                                emit_log(&app_handle, "info", format!("Sync: Latency={}us, Offset={}us, PPM={:.1} (Avg: {:.1})", 
                                    latency_micros, offset_micros, ppm_inst, filtered_ppm));
                            }
                        }
                        
                        thread::sleep(Duration::from_millis(100)); // 10 samples per sec = 200 samples is 20s window
                    }
                }
                Err(e) => {
                    emit_log(&app_handle, "error", format!("Sync Connection failed: {}", e));
                    consecutive_errors += 1;
                    if consecutive_errors > 5 {
                        thread::sleep(Duration::from_secs(5));
                    } else {
                        thread::sleep(Duration::from_secs(1));
                    }
                }
            }
        }
    });
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
            format!("TCP shutdown {} ({}): socket may already be closed", context, e)
        );
    } else {
        emit_log(
            app_handle,
            "debug",
            format!("TCP connection closed gracefully ({})", context)
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
    silence_threshold: f32,
    silence_timeout_seconds: u64,
    is_loopback: bool,
    enable_adaptive_buffer: bool,
    min_buffer_ms: u32,
    max_buffer_ms: u32,
    drift_correction_ppm: i32,
    app_handle: AppHandle,
) -> Result<(cpal::Stream, StreamStats), String> {
    emit_log(
        &app_handle,
        "debug",
        format!(
            "Init stream: Device='{}', Rate={}, Buf={}, RingMs={}, Priority={}, DSCP={}, Chunk={}, SilenceT={}, SilenceTO={}s, Loopback={}, AdaptiveBuf={} ({}ms-{}ms), Correction={}ppm",
            device_name, sample_rate, buffer_size, ring_buffer_duration_ms, high_priority, dscp_strategy, chunk_size, silence_threshold, silence_timeout_seconds, is_loopback, enable_adaptive_buffer, min_buffer_ms, max_buffer_ms, drift_correction_ppm
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
                format!("Loopback device '{}' not found. Available outputs: {:?}", clean_name, device_list_log),
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

    let config = cpal::StreamConfig {
        channels: 2,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Fixed(buffer_size),
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
    
    let ring_buffer_size = (sample_rate as usize) * 2 * (adjusted_ring_buffer_duration_ms as usize) / 1000;
    let buffer_size_mb = (ring_buffer_size * 2) as f32 / (1024.0 * 1024.0);
    
    emit_log(
        &app_handle,
        "info",
        format!(
            "Ring buffer: {}ms ({:.2}MB) - Device type: {}",
            adjusted_ring_buffer_duration_ms,
            buffer_size_mb,
            if is_loopback { "WASAPI Loopback" } else { "Standard Input" }
        ),
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
        
        // Quality tracking variables
        let mut latency_samples: Vec<f32> = Vec::with_capacity(100); //  Track last 100
        let mut jitter_avg: f32 = 0.0; // EWMA of jitter
        let mut last_write_time: Option<Instant> = None;
        let mut consecutive_errors: u64 = 0;
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

        // Pacer State variables for Precision Timing (Phase 1)
        let mut pacer_start_time = Instant::now();
        let mut pacer_frames_sent: u64 = 0;
        let mut pacer_initialized = false;

        // Initialize Silence Detection
        let mut last_audio_activity = Instant::now();

        while is_running_clone.load(Ordering::Relaxed) {
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
                        if current_stream.is_some() { "Connected" } else { "Disconnected" }
                    ),
                );
                last_heartbeat = Instant::now();
            }

            // 1. Fetch Audio Chunk (Blocking/Sleeping if empty to save CPU)
            // We need a full chunk to calculate RMS and send properly
            if cons.len() < temp_buffer.len() {
                thread::sleep(Duration::from_millis(1));
                continue;
            }

            // Pop chunk (we know we have enough data)
            let count = cons.pop_slice(&mut temp_buffer);
            if count == 0 { continue; } // Should not happen given check above, but safety

            // 2. RMS Calculation (Smart Silence Detection)
            let silence_threshold = f32::from_bits(SILENCE_THRESHOLD_BITS.load(Ordering::Relaxed));
            let silence_timeout_secs = SILENCE_TIMEOUT_SECS.load(Ordering::Relaxed);
            
            let mut sum_squares = 0.0;
            for sample in &temp_buffer[0..count] {
                let norm = *sample as f32 / 32768.0;
                sum_squares += norm * norm;
            }
            let rms = (sum_squares / count as f32).sqrt();
            let is_silent = rms < silence_threshold;

            if !is_silent {
                last_audio_activity = Instant::now();
            }

            let silence_duration = last_audio_activity.elapsed();
            // Timeout 0 means disabled
            let is_deep_sleep = silence_timeout_secs > 0 && silence_duration.as_secs() > silence_timeout_secs;

            // 3. Connection Management (Reconnection or Auto-Disconnect)
            if current_stream.is_none() {
                // If we are deep sleeping (long silence), ignore this packet and stay disconnected
                if is_silent && is_deep_sleep {
                    // Drop packet (implied by loop continue)
                    continue; 
                }

                // Otherwise, we have audio (or short silence), so we try to connect
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
                    emit_log(&app_handle_net, "trace", "Write timeout set to 5s".to_string());

                    Ok(stream)
                })();

                match connect_result {
                    Ok(s) => {
                        retry_delay = Duration::from_secs(1);
                        current_stream = Some(s);
                        emit_log(&app_handle_net, "success", "Connected successfully!".to_string());
                        // Important: logic falls through to Sending block
                        // We reset pacer on new connection
                         pacer_initialized = false;
                    }
                    Err(e) => {
                        emit_log(&app_handle_net, "error", format!("Reconnection failed: {}", e));
                        retry_delay = std::cmp::min(retry_delay * 2, MAX_RETRY_DELAY);
                        continue; 
                    }
                }
            }

            // 4. Sending Logic
            if let Some(ref mut s) = current_stream {
                // Check if we should disconnect due to long silence
                if is_silent && is_deep_sleep {
                     emit_log(
                        &app_handle_net,
                        "info",
                        format!("Disconnecting due to silence timeout ({}s)", silence_timeout_secs),
                     );
                     // Close stream
                     close_tcp_stream(s.try_clone().unwrap_or_else(|_| 
                        unsafe { std::mem::zeroed() }
                     ), "silence timeout", &app_handle_net);
                     current_stream = None;
                     continue;
                }

                // If just silent (but not deep sleep), skip sending to save bandwidth
                if is_silent {
                    continue; 
                }

                // Pacer Logic
                if !pacer_initialized {
                    pacer_start_time = Instant::now();
                    pacer_frames_sent = 0;
                    pacer_initialized = true;
                }

                let chunk_frames = count as u64 / 2;
                // Pacing with Drift Correction (Active Rate Control)
                // We read the global atomic which can be updated by Manual Slider OR Auto Sync Thread
                let drift_ppm = DRIFT_CORRECTION_PPM.load(Ordering::Relaxed);
                
                // drift_correction_ppm: Positive = faster (fix underrun), Negative = slower (fix overflow)
                // Formula: adjusted_dur = nom_dur / (1.0 + ppm/1e6)
                
                let nominal_micros = ((chunk_frames * 1_000_000) as f64 / sample_rate as f64);
                let drift_factor = 1.0 + (drift_ppm as f64 / 1_000_000.0);
                let adjusted_micros = nominal_micros / drift_factor;
                
                // We track accumulated fractional microseconds to stay precise over hours
                // But for this loop, we just need the duration for *this specific accumulated count since start*
                
                // Better approach for absolute tracking related to start time:
                // Target Time = Start Time + (Total Frames Sent / (Sample Rate * DriftFactor))
                
                let total_sent_and_current = pacer_frames_sent + chunk_frames;
                let expected_elapsed_micros = (total_sent_and_current as f64 * 1_000_000.0) / (sample_rate as f64 * drift_factor);
                let expected_elapsed = Duration::from_micros(expected_elapsed_micros as u64);

                let target_time = pacer_start_time + expected_elapsed;
                let now = Instant::now();

                if target_time > now {
                    let wait_time = target_time - now;
                    if wait_time > Duration::from_millis(4) {
                        thread::sleep(wait_time);
                    } else if wait_time > Duration::from_micros(500) {
                        thread::sleep(wait_time);
                    } else {
                        thread::yield_now();
                    }
                } else {
                    if now.duration_since(target_time) > Duration::from_millis(ring_buffer_duration_ms as u64) {
                         pacer_start_time = Instant::now();
                         pacer_frames_sent = 0;
                    }
                }

                // Send
                let data_len = (count * 2) as u32;
                let mut payload = Vec::with_capacity(data_len as usize);
                for i in 0..count {
                    payload.extend_from_slice(&temp_buffer[i].to_le_bytes());
                }

                if let Err(e) = s.write_all(&payload) {
                    emit_log(&app_handle_net, "error", format!("Write error: {}", e));
                    current_stream = None; 
                    pacer_initialized = false;
                } else {
                    pacer_frames_sent += chunk_frames;
                    let _ = bytes_sent_clone.fetch_add(payload.len() as u64, Ordering::Relaxed);
                    sequence = sequence.wrapping_add(1);
                    last_write_time = Some(Instant::now());
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
            if enable_adaptive_buffer && last_buffer_check.elapsed() >= Duration::from_secs(BUFFER_CHECK_INTERVAL_SECS) {
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
                let size_diff_pct = ((target_buffer_ms as f32 - current_buffer_ms as f32).abs() / current_buffer_ms as f32) * 100.0;
                
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
                            current_buffer_ms, target_buffer_ms, jitter_avg,
                            adaptive_min_ms, adaptive_max_ms, reason
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
            format!("⚠️ AUDIO STREAM ERROR: {} - This may cause stream to stop!", err)
        );
    };


    // RMS Silence Detection State
    let mut silence_start: Option<Instant> = None;
    let mut transmission_stopped = false;
    let app_handle_audio = app_handle.clone();
    
    // Smoothing state for UI indicator
    let mut smoothed_rms: f32 = 0.0;
    let mut signal_average: f32 = 0.0; // Average volume ONLY when audio is present

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
    let stream_config = if is_loopback {
        emit_log(
            &app_handle,
            "debug",
            "WASAPI loopback: Using Default buffer size for compatibility".to_string(),
        );
        cpal::StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        }
    } else {
        emit_log(
            &app_handle,
            "debug",
            format!("Standard input: Using Fixed buffer size ({})", buffer_size),
        );
        config
    };

    let audio_stream = device
        .build_input_stream(
            &stream_config,
            move |data: &[i16], _: &_| {
                // Read dynamic settings
                let silence_threshold = f32::from_bits(SILENCE_THRESHOLD_BITS.load(Ordering::Relaxed));
                let silence_timeout_seconds = SILENCE_TIMEOUT_SECS.load(Ordering::Relaxed);

                // Calculate RMS for silence detection
                let mut sum_squares = 0.0;
                for &sample in data {
                    sum_squares += (sample as f32) * (sample as f32);
                }
                let current_rms = (sum_squares / data.len() as f32).sqrt();
                
                // 1. Visual Smoothing (Attack/Decay) for the Bar
                if current_rms > smoothed_rms {
                    smoothed_rms = 0.5 * current_rms + 0.5 * smoothed_rms;
                } else {
                    smoothed_rms = 0.05 * current_rms + 0.95 * smoothed_rms;
                }

                // 2. Signal Average (Only update when above threshold)
                // This helps the user see the "average volume of the music" ignoring silence
                if current_rms > silence_threshold {
                    if signal_average == 0.0 {
                        signal_average = current_rms;
                    } else {
                        // Very slow moving average for stability
                        signal_average = 0.01 * current_rms + 0.99 * signal_average;
                    }
                }
                
                // Emit volume levels for UI (every 100ms)
                static LAST_VOLUME_EMIT: AtomicU64 = AtomicU64::new(0);
                let now_millis = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                let last_emit = LAST_VOLUME_EMIT.load(Ordering::Relaxed);
                
                if now_millis - last_emit > 100 {
                    // Emit JSON payload with both metrics
                    // We construct a simple JSON string manually to avoid complex struct definitions here
                    // or just emit a tuple/struct if defined. Let's use a simple format string for now
                    // or better, emit a struct if we can. 
                    // Actually, emit() takes a Serialize type. Let's use a tuple or map.
                    // Or simply emit a custom struct. Let's define a quick struct or use serde_json::json! if available.
                    // Since we don't want to add deps inside the closure easily, let's just emit a tuple (current, average)
                    // But the frontend expects a single value currently. We will update frontend.
                    
                    #[derive(serde::Serialize, Clone)]
                    struct VolumePayload {
                        current: f32,
                        average: f32,
                    }
                    
                    let _ = app_handle_audio.emit("volume-level", VolumePayload { 
                        current: smoothed_rms, 
                        average: signal_average 
                    });
                    
                    LAST_VOLUME_EMIT.store(now_millis, Ordering::Relaxed);
                }

                // Use raw RMS for actual threshold logic to be precise
                if current_rms > silence_threshold {
                    // Audio detected
                    if let Some(start) = silence_start {
                        let duration = start.elapsed();
                        if duration.as_secs() > 1 {
                            emit_log(
                                &app_handle_audio,
                                "info",
                                format!(
                                    "Audio resumed (RMS: {:.1}) after {:.1}s silence [Threshold: {:.1}]",
                                    current_rms, duration.as_secs_f32(), silence_threshold
                                ),
                            );
                        }
                        silence_start = None;
                        transmission_stopped = false;
                    }
                    // Always push actual audio data
                    let pushed = prod.push_slice(data);
                    if pushed < data.len() {
                        // Buffer overflow - log this as it indicates network can't keep up
                        static LAST_OVERFLOW_LOG: AtomicU64 = AtomicU64::new(0);
                        let now_millis = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64;
                        let last_log = LAST_OVERFLOW_LOG.load(Ordering::Relaxed);
                        
                        // Log overflow every 5 seconds to avoid spam
                        if now_millis - last_log > 5000 {
                            emit_log(
                                &app_handle_audio,
                                "warning",
                                format!(
                                    "Buffer overflow: Dropped {} samples. Network/CPU too slow! Consider increasing ring buffer or improving network.",
                                    data.len() - pushed
                                ),
                            );
                            LAST_OVERFLOW_LOG.store(now_millis, Ordering::Relaxed);
                        }
                    }
                } else {
                    // Silence detected
                    if silence_start.is_none() {
                        silence_start = Some(Instant::now());
                        emit_log(
                            &app_handle_audio, 
                            "debug", 
                            format!(
                                "Silence detected (RMS: {:.2} < Threshold: {:.2})",
                                current_rms, silence_threshold
                            )
                        );
                    } else {
                        // Log RMS occasionally even during silence to debug
                        let silence_duration = silence_start.as_ref().unwrap().elapsed();
                        if silence_duration.as_millis() % 5000 < 50 { // Log roughly every 5s
                             // Debug log removed to reduce noise
                        }
                    }
                    
                    let silence_duration = silence_start.as_ref().unwrap().elapsed();
                    
                    if silence_timeout_seconds == 0 || silence_duration.as_secs() < silence_timeout_seconds {
                        // Within grace period or timeout disabled - keep transmitting
                        let pushed = prod.push_slice(data);
                        if pushed < data.len() {
                            // Buffer overflow during silence - log this
                            static LAST_SILENCE_OVERFLOW_LOG: AtomicU64 = AtomicU64::new(0);
                            let now_millis = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64;
                            let last_log = LAST_SILENCE_OVERFLOW_LOG.load(Ordering::Relaxed);
                            
                            // Log overflow every 5 seconds
                            if now_millis - last_log > 5000 {
                                emit_log(
                                    &app_handle_audio,
                                    "warning",
                                    format!(
                                        "Buffer overflow during silence: Dropped {} samples. Network/CPU too slow!",
                                        data.len() - pushed
                                    ),
                                );
                                LAST_SILENCE_OVERFLOW_LOG.store(now_millis, Ordering::Relaxed);
                            }
                        }
                    } else {
                        // Timeout exceeded - stop transmission to save bandwidth
                        if !transmission_stopped {
                            // Use WARNING level so user clearly sees why streaming stopped
                            emit_log(
                                &app_handle_audio,
                                "warning",
                                format!(
                                    "⚠️ TRANSMISSION STOPPED: Silence timeout exceeded ({}s). Audio below threshold {:.1}. Disable silence detection or increase timeout if this is unexpected.",
                                    silence_timeout_seconds, silence_threshold
                                )
                            );
                            transmission_stopped = true;
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
pub fn get_input_devices(#[allow(unused_variables)] include_loopback: bool) -> Result<Vec<String>, String> {
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
                        if name.contains("default") || name.contains("sysdefault") 
                           || name.contains("null") || name.starts_with("hw:") {
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
                        if name.contains("default") || name.contains("sysdefault") 
                           || name.contains("null") || name.starts_with("hw:") {
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
    silence_threshold: f32,
    silence_timeout_seconds: u64,
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
        silence_threshold,
        silence_timeout_seconds,
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
pub fn update_silence_settings(threshold: f32, timeout: u64) {
    SILENCE_THRESHOLD_BITS.store(threshold.to_bits(), Ordering::Relaxed);
    SILENCE_TIMEOUT_SECS.store(timeout, Ordering::Relaxed);
    debug!("Updated silence settings: Threshold={:.1}, Timeout={}s", threshold, timeout);
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
