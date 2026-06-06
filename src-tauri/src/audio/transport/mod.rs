//! Network transport abstraction.
//!
//! `Connection` is the seam the audio send-loop writes through. Today the only
//! implementation is TCP; Phase 2 adds TLS and multi-client behind the same
//! trait without touching the engine.

pub mod dscp;
pub mod tcp_client;
pub mod tcp_server;

use crate::audio::metrics::RttSample;
use std::io::Write;
use std::net::{Shutdown, TcpStream};

/// A writable, RTT-observable stream connection.
pub trait Connection: Write + Send {
    /// Best-effort smoothed RTT for metrics; `None` when unavailable.
    fn rtt(&self) -> Option<RttSample>;
    /// Human-readable peer description for logs.
    fn peer(&self) -> String;
    /// Graceful shutdown (sends TCP FIN). Idempotent / best-effort.
    fn close(&mut self);
}

/// TCP implementation of [`Connection`].
pub struct TcpConnection {
    pub stream: TcpStream,
    peer: String,
}

impl TcpConnection {
    pub fn new(stream: TcpStream) -> Self {
        let peer = stream
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        Self { stream, peer }
    }
}

impl Write for TcpConnection {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

impl Connection for TcpConnection {
    fn rtt(&self) -> Option<RttSample> {
        crate::audio::metrics::tcp_rtt(&self.stream)
    }
    fn peer(&self) -> String {
        self.peer.clone()
    }
    fn close(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}
