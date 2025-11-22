// Connection health monitoring
use serde::Serialize;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Connection health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum HealthStatus {
    Excellent, // Everything working perfectly
    Good,      // Minor issues, still functional
    Degraded,  // Noticeable issues, may affect quality
    Poor,      // Severe issues, dropouts likely
}

/// Health metrics
#[derive(Clone, Serialize)]
pub struct HealthMetrics {
    pub status: HealthStatus,
    pub packets_sent: u64,
    pub packets_per_sec: f64,
    pub ring_buffer_fill_percent: f32,
    pub tcp_write_failures: u64,
    pub avg_write_latency_ms: f32,
    pub buffer_latency_ms: f32,
    pub suggested_action: String,
}

/// Health monitor
pub struct HealthMonitor {
    packets_sent: Arc<AtomicU64>,
    tcp_failures: Arc<AtomicU64>,
    ring_buffer_size: usize,
    ring_buffer_used: Arc<AtomicUsize>,
    last_check: Instant,
    last_packet_count: u64,
    sample_rate: u32,
    channels: u16,
}

impl HealthMonitor {
    pub fn new(ring_buffer_size: usize, sample_rate: u32, channels: u16) -> Self {
        Self {
            packets_sent: Arc::new(AtomicU64::new(0)),
            tcp_failures: Arc::new(AtomicU64::new(0)),
            ring_buffer_size,
            ring_buffer_used: Arc::new(AtomicUsize::new(0)),
            last_check: Instant::now(),
            last_packet_count: 0,
            sample_rate,
            channels,
        }
    }

    pub fn increment_packets(&self) {
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_failures(&self) {
        self.tcp_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn update_buffer_used(&self, used: usize) {
        self.ring_buffer_used.store(used, Ordering::Relaxed);
    }

    pub fn get_metrics(&mut self) -> HealthMetrics {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_check).as_secs_f64();

        let packets_sent = self.packets_sent.load(Ordering::Relaxed);
        let packets_per_sec = if elapsed > 0.0 {
            (packets_sent - self.last_packet_count) as f64 / elapsed
        } else {
            0.0
        };

        self.last_check = now;
        self.last_packet_count = packets_sent;

        let ring_buffer_used = self.ring_buffer_used.load(Ordering::Relaxed);
        let ring_buffer_fill_percent =
            (ring_buffer_used as f32 / self.ring_buffer_size as f32) * 100.0;

        // Calculate buffer latency
        // 16-bit audio = 2 bytes per sample
        let bytes_per_frame = self.channels as u32 * 2;
        let bytes_per_second = self.sample_rate * bytes_per_frame;
        let buffer_latency_ms = if bytes_per_second > 0 {
            (ring_buffer_used as f32 / bytes_per_second as f32) * 1000.0
        } else {
            0.0
        };

        let tcp_failures = self.tcp_failures.load(Ordering::Relaxed);

        // Determine health status
        let status = if tcp_failures > 10 || ring_buffer_fill_percent > 90.0 {
            HealthStatus::Poor
        } else if tcp_failures > 5 || ring_buffer_fill_percent > 70.0 {
            HealthStatus::Degraded
        } else if tcp_failures > 0 || ring_buffer_fill_percent > 50.0 {
            HealthStatus::Good
        } else {
            HealthStatus::Excellent
        };

        // Determine suggested action
        let suggested_action = match status {
            HealthStatus::Poor => "Reduce Bitrate (Critical)",
            HealthStatus::Degraded => "Reduce Bitrate",
            HealthStatus::Good => "Monitor",
            HealthStatus::Excellent => "Maintain",
        }
        .to_string();

        HealthMetrics {
            status,
            packets_sent,
            packets_per_sec,
            ring_buffer_fill_percent,
            tcp_write_failures: tcp_failures,
            avg_write_latency_ms: 0.0, // TODO: Implement actual measurement
            buffer_latency_ms,
            suggested_action,
        }
    }

    // Clone methods removed as they were unused
}
