//! Target address resolution: hostname / IPv4 / IPv6 literal → socket addresses.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs};

/// Resolves `host:port` to one or more socket addresses. `host` may be a
/// hostname, an IPv4 literal, or an IPv6 literal. Errors if nothing resolves.
pub fn resolve_target(host: &str, port: u16) -> io::Result<Vec<SocketAddr>> {
    let host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    let addrs: Vec<SocketAddr> = (host, port).to_socket_addrs()?.collect();
    if addrs.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no addresses resolved for {host}"),
        ));
    }
    Ok(addrs)
}

/// Resolve outside the audio worker through one bounded process-lifetime DNS
/// worker. Cancellation never waits for an operating-system DNS call to finish.
pub fn resolve_target_cancellable(
    host: &str,
    port: u16,
    running: &std::sync::atomic::AtomicBool,
    timeout: std::time::Duration,
) -> io::Result<Vec<SocketAddr>> {
    use std::sync::atomic::Ordering;
    use std::sync::{mpsc, OnceLock};
    use std::time::{Duration, Instant};
    type Request = (String, u16, mpsc::Sender<io::Result<Vec<SocketAddr>>>);
    static RESOLVER: OnceLock<mpsc::SyncSender<Request>> = OnceLock::new();
    if !running.load(Ordering::Relaxed) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Resolution cancelled",
        ));
    }
    let host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, port)]);
    }
    let tx = RESOLVER.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel::<Request>(2);
        std::thread::spawn(move || {
            for (host, port, reply) in rx {
                let _ = reply.send(resolve_target(&host, port));
            }
        });
        tx
    });
    let (reply, result) = mpsc::channel();
    tx.try_send((host.to_string(), port, reply))
        .map_err(|_| io::Error::new(io::ErrorKind::WouldBlock, "DNS resolver busy"))?;
    let end = Instant::now() + timeout;
    loop {
        if !running.load(Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Resolution cancelled",
            ));
        }
        let left = end
            .checked_duration_since(Instant::now())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "DNS resolution timed out"))?;
        match result.recv_timeout(left.min(Duration::from_millis(20))) {
            Ok(value) => return value,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "DNS resolver unavailable",
                ))
            }
        }
    }
}

pub fn resolve_endpoint_cancellable(
    endpoint: &str,
    running: &std::sync::atomic::AtomicBool,
    timeout: std::time::Duration,
) -> io::Result<Vec<SocketAddr>> {
    let (host, port) = endpoint.rsplit_once(':').ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Expected host:port or [IPv6]:port",
        )
    })?;
    let port: u16 = port
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid port"))?;
    if port == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Port must be nonzero",
        ));
    }
    let host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    resolve_target_cancellable(host, port, running, timeout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn cancelled_resolution_does_not_enqueue_dns() {
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let result = resolve_target_cancellable(
            "example.invalid",
            80,
            &cancelled,
            std::time::Duration::from_secs(5),
        );
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
    }
    #[test]
    fn resolves_ipv4_literal() {
        let addrs = resolve_target("127.0.0.1", 4953).unwrap();
        assert!(addrs
            .iter()
            .any(|a| a.ip() == "127.0.0.1".parse::<IpAddr>().unwrap() && a.port() == 4953));
    }

    #[test]
    fn resolves_ipv6_literal() {
        assert_eq!(
            resolve_target("[::1]", 4953).unwrap(),
            resolve_target("::1", 4953).unwrap()
        );
        let addrs = resolve_target("::1", 4953).unwrap();
        assert!(addrs
            .iter()
            .any(|a| a.ip() == "::1".parse::<IpAddr>().unwrap()));
    }
}
