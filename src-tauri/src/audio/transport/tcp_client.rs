//! TCP client connection setup.

use socket2::{Domain, Protocol, Socket, TcpKeepalive, Type};
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// Connects to `ip:port` with audio-friendly socket options and the requested
/// DSCP marking. Returns a ready-to-write blocking `TcpStream`.
pub fn connect(
    ip: &str,
    port: u16,
    dscp_strategy: &str,
    connect_timeout: Duration,
) -> io::Result<TcpStream> {
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
    let _ = socket.set_send_buffer_size(32 * 1024);
    let _ = socket.set_nodelay(true);
    let keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(10))
        .with_interval(Duration::from_secs(1));
    let _ = socket.set_tcp_keepalive(&keepalive);

    let tos = u32::from(super::dscp::dscp_to_tos(dscp_strategy));
    if tos > 0 {
        let _ = socket.set_tos(tos);
    }

    let addr: SocketAddr = format!("{ip}:{port}")
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("invalid address: {e}")))?;
    socket.connect_timeout(&addr.into(), connect_timeout)?;
    Ok(socket.into())
}
