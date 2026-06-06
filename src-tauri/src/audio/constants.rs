use std::time::Duration;

/// Interval between adaptive buffer health checks
pub(crate) const BUFFER_CHECK_INTERVAL_SECS: u64 = 10;

/// Maximum delay between reconnection attempts
pub(crate) const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);

/// How long to wait after disconnect before flushing the ring buffer
pub(crate) const DISCONNECT_FLUSH_THRESHOLD: Duration = Duration::from_secs(3);

/// Interval between heartbeat/stats emissions to the frontend
pub(crate) const HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// Minimum ring buffer duration for loopback devices (ms)
pub(crate) const LOOPBACK_MIN_BUFFER_MS: u32 = 8000;

/// Minimum ring buffer duration for non-loopback devices (ms)
pub(crate) const DEFAULT_MIN_BUFFER_MS: u32 = 5000;

/// Prefill duration as a fraction of sample rate (N in 1/N gives fraction of a second)
pub(crate) const PREFILL_FRACTION: usize = 5;

/// Initial reconnection delay
pub(crate) const INITIAL_RETRY_DELAY_SECS: u64 = 2;

/// Minimum retry delay (after jitter)
pub(crate) const MIN_RETRY_DELAY_MS: u64 = 2000;

/// Minimum adaptive buffer for loopback (ms)
pub(crate) const ADAPTIVE_MIN_LOOPBACK_MS: u32 = 4000;

/// Minimum adaptive buffer for non-loopback (ms)
pub(crate) const ADAPTIVE_MIN_DEFAULT_MS: u32 = 2000;

/// Adaptive buffer adjustment step (ms)
pub(crate) const ADAPTIVE_BUFFER_STEP_MS: u32 = 500;

/// Quality reporting interval (seconds)
pub(crate) const QUALITY_REPORT_INTERVAL_SECS: u64 = 5;

/// Channel capacity for audio command queue
pub(crate) const COMMAND_CHANNEL_CAPACITY: usize = 16;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_are_positive() {
        assert!(BUFFER_CHECK_INTERVAL_SECS > 0);
        assert!(HEARTBEAT_INTERVAL_SECS > 0);
        assert!(COMMAND_CHANNEL_CAPACITY > 0);
        assert!(PREFILL_FRACTION > 0);
    }

    #[test]
    fn test_buffer_constants_ordering() {
        assert!(ADAPTIVE_MIN_DEFAULT_MS < ADAPTIVE_MIN_LOOPBACK_MS,
            "Loopback devices should have larger min buffer");
        assert!(DISCONNECT_FLUSH_THRESHOLD.as_secs() < 60,
            "Flush threshold should be reasonable");
    }
}
