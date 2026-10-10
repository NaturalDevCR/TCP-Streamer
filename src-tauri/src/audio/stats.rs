#![deny(clippy::all)]

use chrono::Local;
use log::{debug, error, info, trace, warn};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

// Log Event
#[derive(Clone, Serialize)]
pub struct LogEvent {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Clone, Serialize)]
pub struct QualityEvent {
    pub session_id: u64,
    pub score: u8,
    /// Best-effort smoothed RTT in ms; `None` => "n/a".
    pub rtt_ms: Option<f32>,
    /// Best-effort RTT variance in ms; `None` => "n/a".
    pub rtt_var_ms: Option<f32>,
    pub underruns: u64,
    pub dropped: u64,
    pub counter_unit: String,
    pub buffer_health: f32,
}

#[derive(Clone, Serialize)]
pub struct StatsEvent {
    pub session_id: u64,
    pub received: bool,
    pub buffer_ms: f32,
    pub target_ms: u32,
    pub sample_rate: u32,
    pub channels: u16,
    pub auth_failures: u64,
    pub lost_frames: u64,
    pub trim_ppm: i32,
    pub uptime_seconds: u64,
    pub bytes_sent: u64,
    pub bitrate_kbps: f64,
}

#[derive(Clone, Serialize)]
pub struct BufferResizeEvent {
    pub session_id: u64,
    pub new_size_ms: u32,
    pub reason: String,
}

// Statistics tracker
pub struct StreamStats {
    pub workers: Vec<std::thread::JoinHandle<()>>,
    #[allow(dead_code)]
    pub bytes_sent: Arc<AtomicU64>,
    #[allow(dead_code)]
    pub start_time: Instant,
    pub is_running: Arc<AtomicBool>,
    /// Samples dropped because the ring buffer was full when the capture
    /// callback tried to push (real overrun signal).
    #[allow(dead_code)]
    pub overruns: Arc<AtomicU64>,
    /// Times the consumer was starved (buffer below one chunk after prefill).
    #[allow(dead_code)]
    pub underruns: Arc<AtomicU64>,
}

impl Drop for StreamStats {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

// Helper function to emit log events
pub fn emit_log(app: &AppHandle, level: &str, message: String) {
    // Global rate limiter for logs
    // Allows max 5 logs per second to prevent flooding the main thread
    static LOG_COUNTER: AtomicUsize = AtomicUsize::new(0);
    static LAST_LOG_RESET: AtomicU64 = AtomicU64::new(0);

    // Also log to standard logger (terminal/file) - always log to stdout
    match level {
        "error" => error!("{}", message),
        "warning" => warn!("{}", message),
        "info" => info!("{}", message),
        "debug" => debug!("{}", message),
        "trace" => trace!("{}", message),
        "success" => info!("SUCCESS: {}", message), // Map success to info
        _ => info!("[{}] {}", level, message),
    }

    let now_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let last_reset = LAST_LOG_RESET.load(Ordering::Relaxed);

    if now_millis.saturating_sub(last_reset) > 1000 {
        // Reset counter every second
        LAST_LOG_RESET.store(now_millis, Ordering::Relaxed);
        LOG_COUNTER.store(0, Ordering::Relaxed);
    }

    // Only emit to Frontend if under limit
    if level == "error" || level == "warning" || LOG_COUNTER.fetch_add(1, Ordering::Relaxed) < 5 {
        let log = LogEvent {
            timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            level: level.to_string(),
            message,
        };
        let _ = app.emit("log-event", log);
    }
}

#[derive(Default)]
pub struct ReceiveMetrics {
    pub wire_format: AtomicU64,
    pub bytes_received: AtomicU64,
    pub lost_frames: AtomicU64,
    pub auth_failures: AtomicU64,
    pub received_packets: AtomicU64,
    pub reconnects: AtomicU64,
    pub buffered_samples: AtomicU64,
}
