//! Network transport abstraction.
//!
//! `Connection` is the seam the audio send-loop writes through. Today the only
//! implementation is TCP; Phase 2 adds TLS and multi-client behind the same
//! trait without touching the engine.

pub mod allowlist;
pub mod discovery;
pub mod dscp;
pub mod resolve;
pub mod tcp_client;
pub mod tcp_server;
pub mod udp;

use crate::audio::metrics::RttSample;
use std::io::Write;
use std::net::{Shutdown, TcpStream};

/// A writable, RTT-observable stream connection.
#[allow(dead_code)]
pub trait Connection: Write + Send {
    /// Best-effort smoothed RTT for metrics; `None` when unavailable.
    fn rtt(&self) -> Option<RttSample>;
    /// Human-readable peer description for logs.
    fn peer(&self) -> String;
    /// Graceful shutdown (sends TCP FIN). Idempotent / best-effort.
    fn close(&mut self);
}

/// TCP implementation of [`Connection`].
#[allow(dead_code)]
pub struct TcpConnection {
    pub stream: TcpStream,
    peer: String,
}

impl TcpConnection {
    #[allow(dead_code)]
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
        self.stream.set_nonblocking(true)?;
        self.stream.write(buf)
    }
    fn write_all(&mut self, mut buf: &[u8]) -> std::io::Result<()> {
        use std::io::{Error, ErrorKind};
        use std::time::{Duration, Instant};
        self.stream.set_nonblocking(true)?;
        let end = Instant::now() + Duration::from_millis(200);
        while !buf.is_empty() {
            let left = end
                .checked_duration_since(Instant::now())
                .ok_or_else(|| Error::new(ErrorKind::TimedOut, "Audio write deadline exceeded"))?;
            let _ = left;
            match self.stream.write(buf) {
                Ok(0) => return Err(Error::new(ErrorKind::WriteZero, "Peer accepted no audio")),
                Ok(n) => buf = &buf[n..],
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(2))
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slow_receiver_cannot_extend_audio_write_deadline() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        // Limit the advertised receive window before the handshake. Windows can
        // otherwise accept a large loopback write despite a small send buffer.
        socket2::SockRef::from(&listener)
            .set_recv_buffer_size(1024)
            .unwrap();
        let stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (peer, _) = listener.accept().unwrap();
        let socket = socket2::SockRef::from(&stream);
        socket.set_send_buffer_size(1024).unwrap();
        let mut connection = TcpConnection::new(stream);
        // Observe actual backpressure instead of assuming an OS buffer size.
        let payload = vec![0; 64 * 1024];
        let mut filled = 0;
        let fill_start = std::time::Instant::now();
        loop {
            match connection.write(&payload) {
                Ok(n) if n > 0 => filled += n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                other => panic!("Unexpected socket prefill result: {other:?}"),
            }
            assert!(
                filled < 64 * 1024 * 1024,
                "Loopback never applied backpressure"
            );
            assert!(fill_start.elapsed() < std::time::Duration::from_secs(2));
        }
        let start = std::time::Instant::now();
        let result = connection.write_all(&vec![0; 8 * 1024 * 1024]);
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
        connection.close();
        drop(peer);
    }
}
