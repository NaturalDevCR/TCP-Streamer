use std::time::Duration;

/// Interval between adaptive buffer health checks
#[allow(dead_code)]
pub(crate) const BUFFER_CHECK_INTERVAL_SECS: u64 = 10;

/// Maximum delay between reconnection attempts
pub(crate) const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);

/// How long to wait after disconnect before flushing the ring buffer
pub(crate) const DISCONNECT_FLUSH_THRESHOLD: Duration = Duration::from_secs(3);

/// Interval between heartbeat/stats emissions to the frontend
pub(crate) const HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// Initial reconnection delay
pub(crate) const INITIAL_RETRY_DELAY_SECS: u64 = 2;

/// Minimum retry delay (after jitter)
pub(crate) const MIN_RETRY_DELAY_MS: u64 = 2000;

/// Adaptive buffer adjustment step (ms)
pub(crate) const ADAPTIVE_BUFFER_STEP_MS: u32 = 500;

/// Quality reporting interval (seconds)
pub(crate) const QUALITY_REPORT_INTERVAL_SECS: u64 = 5;

/// Channel capacity for audio command queue
pub(crate) const COMMAND_CHANNEL_CAPACITY: usize = 16;

#[cfg(test)]
#[allow(clippy::assertions_on_constants)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_are_positive() {
        assert!(BUFFER_CHECK_INTERVAL_SECS > 0);
        assert!(HEARTBEAT_INTERVAL_SECS > 0);
        assert!(COMMAND_CHANNEL_CAPACITY > 0);
    }
}
