//! mDNS advertise (source) + browse (sink) for native UDP sources.

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::Serialize;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

const SERVICE_TYPE: &str = "_tcp-streamer._udp.local.";

/// Keeps the mDNS registration alive while held (drop to unregister).
pub struct Advertiser {
    daemon: ServiceDaemon,
    fullname: String,
}

/// Advertises this instance as a native source on the LAN.
pub fn advertise(instance: &str, port: u16, encrypted: bool) -> Result<Advertiser, String> {
    let ip = local_ip_address::local_ip().map_err(|e| e.to_string())?;
    let label: String = instance
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(63)
        .collect();
    let label = label.trim_matches('-');
    let host = format!(
        "{}.local.",
        if label.is_empty() {
            "tcp-streamer"
        } else {
            label
        }
    );
    let enc = if encrypted { "1" } else { "0" };
    let props = [("ver", "2"), ("enc", enc)];
    let info = ServiceInfo::new(SERVICE_TYPE, instance, &host, ip, port, &props[..])
        .map_err(|e| e.to_string())?
        .enable_addr_auto();
    let fullname = info.get_fullname().to_string();
    let daemon = ServiceDaemon::new().map_err(|e| e.to_string())?;
    if let Err(error) = daemon.register(info) {
        shutdown(&daemon);
        return Err(error.to_string());
    }
    Ok(Advertiser { daemon, fullname })
}

impl Drop for Advertiser {
    fn drop(&mut self) {
        if let Ok(ack) = self.daemon.unregister(&self.fullname) {
            let _ = ack.recv_timeout(Duration::from_millis(100));
        }
        shutdown(&self.daemon);
    }
}

fn shutdown(daemon: &ServiceDaemon) {
    if let Ok(ack) = daemon.shutdown() {
        let _ = ack.recv_timeout(Duration::from_millis(100));
    }
}

/// A discovered native source.
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredSource {
    pub name: String,
    pub addr: String, // ip:port
    pub encrypted: bool,
}

/// Browses for native sources for `timeout`, returning what resolved.
pub fn browse(timeout: Duration) -> Result<Vec<DiscoveredSource>, String> {
    let daemon = ServiceDaemon::new().map_err(|e| e.to_string())?;
    let rx = match daemon.browse(SERVICE_TYPE) {
        Ok(rx) => rx,
        Err(error) => {
            shutdown(&daemon);
            return Err(error.to_string());
        }
    };
    let deadline = Instant::now() + timeout;
    let mut out = Vec::new();
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(remaining) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                if info.get_property_val_str("ver") != Some("2") {
                    continue;
                }
                let encrypted = info.get_property_val_str("enc") == Some("1");
                for ip in info.get_addresses() {
                    // Bare mDNS IPv6 link-local addresses lack interface scopes;
                    // they cannot be used reliably as a connect target.
                    if matches!(ip, std::net::IpAddr::V6(ip) if ip.is_unicast_link_local()) {
                        continue;
                    }
                    let addr = SocketAddr::new(*ip, info.get_port()).to_string();
                    if out
                        .iter()
                        .any(|s: &DiscoveredSource| s.name == info.get_fullname() && s.addr == addr)
                    {
                        continue;
                    }
                    out.push(DiscoveredSource {
                        name: info.get_fullname().to_string(),
                        addr,
                        encrypted,
                    });
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let _ = daemon.stop_browse(SERVICE_TYPE);
    shutdown(&daemon);
    out.sort_by(|a, b| a.name.cmp(&b.name).then(a.addr.cmp(&b.addr)));
    Ok(out)
}
