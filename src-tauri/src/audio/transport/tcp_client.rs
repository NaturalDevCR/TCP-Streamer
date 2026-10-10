//! TCP client connection setup.

use socket2::{Domain, Protocol, Socket, TcpKeepalive, Type};
use std::io;
use std::net::TcpStream;
use std::time::Duration;

/// Connects to `host:port` (hostname, IPv4, or IPv6) with audio-friendly socket
/// options and the requested DSCP marking. Tries each resolved address until one
/// connects. Returns a ready-to-write blocking `TcpStream`.
pub fn connect(
    host: &str,
    port: u16,
    dscp_strategy: &str,
    connect_timeout: Duration,
    running: &std::sync::atomic::AtomicBool,
) -> io::Result<TcpStream> {
    let addrs =
        super::resolve::resolve_target_cancellable(host, port, running, Duration::from_secs(2))?;
    let deadline = std::time::Instant::now() + connect_timeout;
    let tos = u32::from(super::dscp::dscp_to_tos(dscp_strategy));

    let mut last_err: Option<io::Error> = None;
    for addr in addrs {
        if !running.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Connection cancelled",
            ));
        }
        let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) else {
            break;
        };
        let domain = if addr.is_ipv6() {
            Domain::IPV6
        } else {
            Domain::IPV4
        };
        let socket = match Socket::new(domain, Type::STREAM, Some(Protocol::TCP)) {
            Ok(s) => s,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };
        let _ = socket.set_send_buffer_size(32 * 1024);
        let _ = socket.set_nodelay(true);
        let keepalive = TcpKeepalive::new()
            .with_time(Duration::from_secs(10))
            .with_interval(Duration::from_secs(1));
        let _ = socket.set_tcp_keepalive(&keepalive);
        if tos > 0 {
            let _ = socket.set_tos(tos);
        }

        match socket.connect_timeout(&addr.into(), left.min(Duration::from_millis(100))) {
            Ok(()) => {
                let stream: TcpStream = socket.into();
                stream.set_write_timeout(Some(Duration::from_millis(200)))?;
                stream.set_read_timeout(Some(Duration::from_millis(200)))?;
                return Ok(stream);
            }
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            "no address could be connected",
        )
    }))
}
