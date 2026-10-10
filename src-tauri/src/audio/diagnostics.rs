//! Non-streaming preflight. Device metadata and DNS can block, so run off the IPC thread.
use super::engine::device::{negotiate, ConfigCandidate, SampleFmt};
use super::engine::latency::{validate_overrides, LatencyOverrides};
use cpal::traits::DeviceTrait;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    #[default]
    Source,
    Sink,
}
#[derive(Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    #[default]
    Tcp,
    Udp,
}
#[derive(Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Client,
    Server,
}
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    UltraLow,
    #[default]
    Balanced,
    Robust,
    Broadcast,
    Custom,
}
impl Profile {
    fn name(self) -> &'static str {
        match self {
            Self::UltraLow => "ultra-low",
            Self::Balanced => "balanced",
            Self::Robust => "robust",
            Self::Broadcast => "broadcast",
            Self::Custom => "custom",
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct PreflightConfig {
    role: Role,
    transport: Transport,
    mode: Mode,
    device: String,
    host: String,
    port: u16,
    source_addr: String,
    sample_rate: u32,
    buffer_size: u32,
    is_loopback: bool,
    latency_profile: Profile,
    overrides: LatencyOverrides,
    allowlist: String,
}
impl Default for PreflightConfig {
    fn default() -> Self {
        Self {
            role: Role::Source,
            transport: Transport::Tcp,
            mode: Mode::Client,
            device: String::new(),
            host: String::new(),
            port: 1704,
            source_addr: String::new(),
            sample_rate: 48000,
            buffer_size: 1024,
            is_loopback: false,
            latency_profile: Profile::Balanced,
            overrides: LatencyOverrides::default(),
            allowlist: String::new(),
        }
    }
}
#[derive(Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StageStatus {
    Ok,
    Error,
    Warning,
}
#[derive(Serialize)]
pub struct Stage {
    name: &'static str,
    status: StageStatus,
    detail: String,
}
#[derive(Serialize)]
pub struct PreflightResult {
    stages: Vec<Stage>,
    ready: bool,
}
fn stage(name: &'static str, result: Result<String, String>) -> Stage {
    match result {
        Ok(detail) => Stage {
            name,
            status: StageStatus::Ok,
            detail,
        },
        Err(detail) => Stage {
            name,
            status: StageStatus::Error,
            detail,
        },
    }
}
#[tauri::command]
pub async fn preflight_stream(config: PreflightConfig) -> Result<PreflightResult, String> {
    tauri::async_runtime::spawn_blocking(move || inspect(config))
        .await
        .map_err(|e| e.to_string())
}
fn inspect(config: PreflightConfig) -> PreflightResult {
    let mut stages = vec![
        stage("configuration", validate_config(&config)),
        stage("device", inspect_device(&config)),
        stage("network", inspect_network(&config)),
    ];
    stages.push(Stage { name: "scope", status: StageStatus::Warning, detail: "No audio device was opened and no connection or audio packet was sent. Permission, remote reachability, encryption and measured latency are confirmed only by starting a stream. Port availability can change after this check.".into() });
    let ready = !stages.iter().any(|s| s.status == StageStatus::Error);
    PreflightResult { stages, ready }
}
fn validate_config(config: &PreflightConfig) -> Result<String, String> {
    validate_overrides(config.latency_profile.name(), config.overrides)?;
    if config.role == Role::Source {
        if config.port == 0
            || !(8000..=192000).contains(&config.sample_rate)
            || config.buffer_size > 65536
        {
            return Err("Invalid port, sample rate or capture buffer".into());
        }
        if config.transport == Transport::Tcp && config.mode == Mode::Server {
            super::transport::allowlist::validate_rules(&config.allowlist)?;
        }
    }
    if config.is_loopback && config.role == Role::Source && !cfg!(target_os = "windows") {
        return Err("Loopback capture requires Windows".into());
    }
    Ok("Configuration and latency limits are valid".into())
}
fn inspect_device(config: &PreflightConfig) -> Result<String, String> {
    if config.device.trim().is_empty() {
        return Err("Select an audio device".into());
    }
    let output = config.role == Role::Sink || config.is_loopback;
    let name = if config.is_loopback && config.role == Role::Source {
        config
            .device
            .strip_prefix("[Loopback] ")
            .unwrap_or(&config.device)
    } else {
        &config.device
    };
    let device = super::engine::device::find_device(name, output)?;
    let ranges: Vec<cpal::SupportedStreamConfigRange> = if output {
        device
            .supported_output_configs()
            .map(|ranges| ranges.collect())
    } else {
        device
            .supported_input_configs()
            .map(|ranges| ranges.collect())
    }
    .map_err(|e| format!("Cannot inspect supported audio ranges: {e}"))?;
    let candidates: Vec<_> = ranges
        .iter()
        .filter_map(|range| {
            let format = match range.sample_format() {
                cpal::SampleFormat::F32 => SampleFmt::F32,
                cpal::SampleFormat::I16 => SampleFmt::I16,
                cpal::SampleFormat::U16 => SampleFmt::U16,
                cpal::SampleFormat::I32 => SampleFmt::I32,
                _ => return None,
            };
            Some(ConfigCandidate {
                channels: range.channels(),
                format,
                min_rate: range.min_sample_rate().0,
                max_rate: range.max_sample_rate().0,
            })
        })
        .collect();
    let selected =
        negotiate(&candidates, config.sample_rate).ok_or("No supported PCM device format")?;
    let chosen = candidates[selected.index];
    let mut detail = format!(
        "Device available: {} channels, {:?}, {} Hz; requested {} Hz",
        chosen.channels, chosen.format, selected.capture_rate, config.sample_rate
    );
    if config.role == Role::Source && config.buffer_size != 0 {
        // Match the engine's negotiation ordering and report the supported range,
        // without opening the device or claiming the driver applied a size.
        let range = ranges
            .iter()
            .filter(|r| {
                matches!(
                    r.sample_format(),
                    cpal::SampleFormat::F32
                        | cpal::SampleFormat::I16
                        | cpal::SampleFormat::U16
                        | cpal::SampleFormat::I32
                )
            })
            .nth(selected.index)
            .ok_or("Supported range changed")?;
        match range.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => detail.push_str(&format!(
                "; capture request {} frames, supported {}–{} (negotiated request {})",
                config.buffer_size,
                min,
                max,
                config.buffer_size.clamp(*min, *max)
            )),
            cpal::SupportedBufferSize::Unknown => {
                detail.push_str("; driver buffer limits are unknown")
            }
        }
    }
    Ok(detail)
}
/// Strict host validation; URLs, paths, whitespace and unbracketed host:port are rejected.
pub(crate) fn validate_host(host: &str) -> Result<&str, String> {
    if host.is_empty()
        || host.len() > 253
        || host.chars().any(char::is_whitespace)
        || host.contains('/')
        || host.contains('@')
    {
        return Err("Enter a host or IP address without a URL, path or spaces".into());
    }
    let normalized = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    if normalized.parse::<IpAddr>().is_ok() {
        return Ok(normalized);
    }
    let domain = normalized.strip_suffix('.').unwrap_or(normalized);
    if domain.split('.').any(|label| {
        label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    }) {
        return Err("Invalid hostname; IPv6 sink addresses require [address]:port".into());
    }
    Ok(normalized)
}
pub(crate) fn parse_endpoint(endpoint: &str) -> Result<(String, u16), String> {
    let (host, port) = endpoint
        .rsplit_once(':')
        .ok_or("Enter source host:port or [IPv6]:port")?;
    if host.contains(':') && !(host.starts_with('[') && host.ends_with(']')) {
        return Err("IPv6 sink addresses require [address]:port".into());
    }
    let port = port
        .parse::<u16>()
        .ok()
        .filter(|p| *p != 0)
        .ok_or("Port must be 1–65535")?;
    Ok((validate_host(host)?.into(), port))
}
fn inspect_network(config: &PreflightConfig) -> Result<String, String> {
    if config.role == Role::Source
        && (config.transport == Transport::Udp || config.mode == Mode::Server)
    {
        if config.port == 0 {
            return Err("Port must be 1–65535".into());
        }
        probe_bind(config.transport, config.port)?;
        return Ok("Local listening port is available now; temporary socket released".into());
    }
    let (host, port) = if config.role == Role::Sink {
        parse_endpoint(&config.source_addr)?
    } else {
        (validate_host(&config.host)?.into(), config.port)
    };
    if port == 0 {
        return Err("Port must be 1–65535".into());
    }
    let addresses = super::transport::resolve::resolve_target_cancellable(
        &host,
        port,
        &std::sync::atomic::AtomicBool::new(true),
        std::time::Duration::from_secs(2),
    )
    .map_err(|e| format!("Cannot resolve endpoint: {e}"))?;
    if addresses.is_empty() {
        return Err("Endpoint resolved to no addresses".into());
    }
    Ok(format!(
        "Endpoint resolves to {} address(es); remote service not contacted",
        addresses.len()
    ))
}
fn probe_bind(transport: Transport, port: u16) -> Result<(), String> {
    let (kind, protocol) = if transport == Transport::Udp {
        (socket2::Type::DGRAM, socket2::Protocol::UDP)
    } else {
        (socket2::Type::STREAM, socket2::Protocol::TCP)
    };
    let bind = |domain, address: SocketAddr| -> std::io::Result<()> {
        let socket = socket2::Socket::new(domain, kind, Some(protocol))?;
        if domain == socket2::Domain::IPV6 {
            socket.set_only_v6(false)?;
        }
        socket.bind(&address.into())?;
        Ok(())
    };
    // Some hosts permit a dual-stack IPv6 bind alongside an occupied IPv4
    // wildcard port. Check IPv4 explicitly first so that ready never hides an
    // existing IPv4 listener; each probe drops its socket before the next one.
    bind(
        socket2::Domain::IPV4,
        SocketAddr::from(([0, 0, 0, 0], port)),
    )
    .map_err(|e| format!("Local IPv4 port cannot be bound: {e}"))?;
    bind(socket2::Domain::IPV6, SocketAddr::from(([0u16; 8], port)))
        .or_else(|_| {
            bind(
                socket2::Domain::IPV4,
                SocketAddr::from(([0, 0, 0, 0], port)),
            )
        })
        .map_err(|e| format!("Local port cannot be bound: {e}"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_urls_missing_ports_and_ambiguous_ipv6() {
        for address in [
            "http://example.com:1704",
            "example.com",
            "::1:1704",
            "host:0",
            "host:65536",
            "host name:1704",
            "host/path:1704",
        ] {
            assert!(parse_endpoint(address).is_err(), "{address}");
        }
        assert_eq!(parse_endpoint("[::1]:1704").unwrap(), ("::1".into(), 1704));
        assert_eq!(
            parse_endpoint("localhost:1704").unwrap(),
            ("localhost".into(), 1704)
        );
    }
    #[test]
    fn udp_port_probe_detects_real_occupied_socket_and_releases_available_socket() {
        let occupied = std::net::UdpSocket::bind("0.0.0.0:0").unwrap();
        let port = occupied.local_addr().unwrap().port();
        assert!(probe_bind(Transport::Udp, port).is_err());
        drop(occupied);
        assert!(probe_bind(Transport::Udp, port).is_ok());
        assert!(std::net::UdpSocket::bind(("0.0.0.0", port)).is_ok());
    }
    #[test]
    fn network_resolution_is_local_and_invalid_config_is_not_ready() {
        let config = PreflightConfig {
            host: "127.0.0.1".into(),
            ..Default::default()
        };
        assert!(inspect_network(&config).is_ok());
        let config = PreflightConfig {
            device: String::new(),
            port: 0,
            ..Default::default()
        };
        assert!(!inspect(config).ready);
    }
    #[test]
    fn strict_enums_reject_unknown_roles_and_profiles() {
        assert!(serde_json::from_str::<PreflightConfig>(r#"{"role":"unknown"}"#).is_err());
        assert!(
            serde_json::from_str::<PreflightConfig>(r#"{"latencyProfile":"unknown"}"#).is_err()
        );
    }
}
