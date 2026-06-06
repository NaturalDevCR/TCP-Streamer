//! Target address resolution: hostname / IPv4 / IPv6 literal → socket addresses.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs};

/// Resolves `host:port` to one or more socket addresses. `host` may be a
/// hostname, an IPv4 literal, or an IPv6 literal. Errors if nothing resolves.
pub fn resolve_target(host: &str, port: u16) -> io::Result<Vec<SocketAddr>> {
    let addrs: Vec<SocketAddr> = (host, port).to_socket_addrs()?.collect();
    if addrs.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no addresses resolved for {host}"),
        ));
    }
    Ok(addrs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn resolves_ipv4_literal() {
        let addrs = resolve_target("127.0.0.1", 4953).unwrap();
        assert!(addrs.iter().any(|a| a.ip() == "127.0.0.1".parse::<IpAddr>().unwrap()
            && a.port() == 4953));
    }

    #[test]
    fn resolves_ipv6_literal() {
        let addrs = resolve_target("::1", 4953).unwrap();
        assert!(addrs.iter().any(|a| a.ip() == "::1".parse::<IpAddr>().unwrap()));
    }
}
