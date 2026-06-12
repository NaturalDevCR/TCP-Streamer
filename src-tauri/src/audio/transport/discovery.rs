//! mDNS advertise (source) + browse (sink) for native UDP sources.

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::Serialize;
use std::time::{Duration, Instant};

const SERVICE_TYPE: &str = "_tcp-streamer._udp.local.";

/// Keeps the mDNS registration alive while held (drop to unregister).
pub struct Advertiser {
    _daemon: ServiceDaemon,
}

/// Advertises this instance as a native source on the LAN.
pub fn advertise(instance: &str, port: u16, encrypted: bool) -> Result<Advertiser, String> {
    let daemon = ServiceDaemon::new().map_err(|e| e.to_string())?;
    let ip = local_ip_address::local_ip().map_err(|e| e.to_string())?;
    let host = format!("{instance}.local.");
    let enc = if encrypted { "1" } else { "0" };
    let props = [("ver", "1"), ("enc", enc)];
    let info = ServiceInfo::new(SERVICE_TYPE, instance, &host, ip, port, &props[..])
        .map_err(|e| e.to_string())?;
    daemon.register(info).map_err(|e| e.to_string())?;
    Ok(Advertiser { _daemon: daemon })
}

/// A discovered native source.
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredSource {
    pub name: String,
    pub addr: String, // ip:port
    pub encrypted: bool,
}

/// Browses for native sources for `timeout`, returning what resolved.
pub fn browse(timeout: Duration) -> Vec<DiscoveredSource> {
    let Ok(daemon) = ServiceDaemon::new() else {
        return Vec::new();
    };
    let Ok(rx) = daemon.browse(SERVICE_TYPE) else {
        return Vec::new();
    };
    let deadline = Instant::now() + timeout;
    let mut out = Vec::new();
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(remaining) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                if let Some(ip) = info.get_addresses_v4().iter().next() {
                    let encrypted = info
                        .get_property_val_str("enc")
                        .map(|v| v == "1")
                        .unwrap_or(false);
                    out.push(DiscoveredSource {
                        name: info.get_fullname().to_string(),
                        addr: format!("{}:{}", ip, info.get_port()),
                        encrypted,
                    });
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    out
}
