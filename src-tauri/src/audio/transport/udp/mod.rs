//! Native low-latency UDP transport (Phase 2B).
pub mod crypto;
pub mod drift;
pub mod jitter;
pub mod packet;
pub mod sink;
pub mod source;
#[cfg(test)]
mod tests_loopback;
