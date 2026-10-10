#![deny(clippy::all)]

use super::error::AudioError;
use super::manager::{AudioCommand, AudioState};
use super::session::SessionStatus;
use cpal::traits::{DeviceTrait, HostTrait};
use log::debug;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, State};

static DEVICE_CACHE: Mutex<Option<(bool, Vec<String>, Instant)>> = Mutex::new(None);
const DEVICE_CACHE_TTL_SECS: u64 = 30;

/// Whether "[Loopback] <output>" pseudo-devices should be offered. WASAPI
/// loopback capture exists only on Windows; listing those entries elsewhere
/// produces phantom devices that capture nothing (and, sorting first, they
/// become the silent default when the saved device disappears).
fn offer_loopback_devices(os: &str, include_loopback: bool) -> bool {
    os == "windows" && include_loopback
}

/// Starts streaming audio from the given device to the specified IP:port.
///
/// Supports both client mode (connects to a remote server) and server mode
/// (listens for incoming connections). Audio is captured via CPAL and sent
/// over TCP as raw PCM or WAV.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn start_stream(
    state: State<'_, AudioState>,
    app_handle: AppHandle,
    device_name: String,
    ip: String,
    port: u16,
    sample_rate: u32,
    buffer_size: u32,
    ring_buffer_duration_ms: u32,
    auto_reconnect: bool,
    high_priority: bool,
    dscp_strategy: String,
    format: String,
    chunk_size: u32,
    is_loopback: bool,
    is_server: bool,
    enable_adaptive_buffer: bool,
    min_buffer_ms: u32,
    max_buffer_ms: u32,
    fixed_latency_ms: u32,
    latency_profile: String,
    allowlist: String,
    transport: String,
    psk: String,
) -> Result<SessionStatus, AudioError> {
    validate_source(
        &device_name,
        &ip,
        port,
        sample_rate,
        &transport,
        is_server,
        &latency_profile,
        &allowlist,
        chunk_size,
        min_buffer_ms,
        max_buffer_ms,
    )?;
    super::engine::latency::validate_overrides(
        &latency_profile,
        super::engine::latency::LatencyOverrides {
            ring_ms: Some(ring_buffer_duration_ms),
            min_buffer_ms: Some(min_buffer_ms),
            max_buffer_ms: Some(max_buffer_ms),
            chunk_size: Some(chunk_size),
            fixed_latency_ms: Some(fixed_latency_ms),
        },
    )
    .map_err(AudioError::NetworkError)?;
    if !["pcm", "wav"].contains(&format.as_str())
        || !["voip", "ef", "cs5", "lowdelay", "throughput", "besteffort"]
            .contains(&dscp_strategy.as_str())
        || buffer_size > 65536
        || psk.len() > 1024
    {
        return Err(AudioError::NetworkError(
            "Invalid format, DSCP, capture buffer or secret length".into(),
        ));
    }
    let (reply, rx) = mpsc::channel();
    let command = AudioCommand::Start {
        device_name,
        ip,
        port,
        sample_rate,
        buffer_size,
        ring_buffer_duration_ms,
        auto_reconnect,
        high_priority,
        dscp_strategy,
        format,
        chunk_size,
        is_loopback,
        is_server,
        enable_adaptive_buffer,
        min_buffer_ms,
        max_buffer_ms,
        fixed_latency_ms,
        latency_profile,
        allowlist,
        transport,
        psk,
        app_handle: Box::new(app_handle),
        reply,
        running: state.new_running_flag(),
    };

    request(&state, command, rx).await
}

/// Stops the currently active audio stream.
#[tauri::command]
pub async fn stop_stream(
    state: State<'_, AudioState>,
    app_handle: AppHandle,
) -> Result<SessionStatus, AudioError> {
    state.cancel();
    let (reply, rx) = mpsc::channel();
    request(
        &state,
        AudioCommand::Stop {
            reply: Some(reply),
            app_handle: Some(Box::new(app_handle)),
        },
        rx,
    )
    .await
}

/// Starts the sink: subscribes to a native UDP source and plays audio
/// to the selected output device.
#[tauri::command]
pub async fn start_sink(
    state: State<'_, AudioState>,
    app_handle: AppHandle,
    output_device: String,
    source_addr: String,
    latency_profile: String,
    overrides: super::engine::latency::LatencyOverrides,
    psk: String,
) -> Result<SessionStatus, AudioError> {
    if output_device.trim().is_empty() || source_addr.trim().is_empty() {
        return Err(AudioError::NetworkError(
            "Select output and source address".into(),
        ));
    }
    validate_profile(&latency_profile)?;
    super::diagnostics::parse_endpoint(&source_addr).map_err(AudioError::NetworkError)?;
    super::engine::latency::validate_overrides(&latency_profile, overrides)
        .map_err(AudioError::NetworkError)?;
    if psk.len() > 1024 {
        return Err(AudioError::NetworkError("Secret exceeds 1024 bytes".into()));
    }
    let (reply, rx) = mpsc::channel();
    request(
        &state,
        AudioCommand::StartSink {
            output_device,
            source_addr,
            latency_profile,
            overrides,
            psk,
            app_handle: Box::new(app_handle),
            reply,
            running: state.new_running_flag(),
        },
        rx,
    )
    .await
}

/// Returns the operating system name (e.g. "macos", "linux", "windows").
#[tauri::command]
pub fn get_os_type() -> String {
    std::env::consts::OS.to_string()
}

/// Enumerates all available audio input devices across all CPAL hosts.
///
/// Results are cached for 30 seconds to reduce repeated system calls.
#[tauri::command]
pub fn get_input_devices(include_loopback: bool) -> Result<Vec<String>, AudioError> {
    let list_loopback = offer_loopback_devices(std::env::consts::OS, include_loopback);

    // Check cache (keyed by the loopback flag so toggling refreshes the list)
    if let Ok(cache) = DEVICE_CACHE.lock() {
        if let Some((cached_flag, cached_devices, timestamp)) = cache.as_ref() {
            if *cached_flag == list_loopback
                && timestamp.elapsed().as_secs() < DEVICE_CACHE_TTL_SECS
            {
                return Ok(cached_devices.clone());
            }
        }
    }

    let mut all_devices = Vec::new();

    // Try all available hosts
    for host_id in cpal::available_hosts() {
        match cpal::host_from_id(host_id) {
            Ok(host) => {
                debug!("Scanning host: {:?}", host_id);
                // 1. Standard Input Devices
                if let Ok(devices) = host.input_devices() {
                    for dev in devices {
                        if let Ok(name) = dev.name() {
                            all_devices.push(name);
                        }
                    }
                }

                // 2. WASAPI Loopback pseudo-devices (Windows only)
                if list_loopback {
                    if let Ok(output_devices) = host.output_devices() {
                        for dev in output_devices {
                            if let Ok(name) = dev.name() {
                                all_devices.push(format!("[Loopback] {}", name));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                debug!("Failed to get host {:?}: {}", host_id, e);
            }
        }
    }

    // Dedup
    all_devices.sort();
    all_devices.dedup();

    // Update cache
    if let Ok(mut cache) = DEVICE_CACHE.lock() {
        *cache = Some((list_loopback, all_devices.clone(), Instant::now()));
    }

    Ok(all_devices)
}

// Stopping the sink reuses `stop_stream` (sends Stop to the manager,
// which stops whatever is currently running).

/// Lists native UDP sources discovered via mDNS on the LAN.
#[tauri::command]
pub fn list_sources() -> Vec<crate::audio::transport::discovery::DiscoveredSource> {
    crate::audio::transport::discovery::browse(std::time::Duration::from_millis(1500))
}

/// Enumerates available audio output devices across all CPAL hosts.
#[tauri::command]
pub fn get_output_devices() -> Result<Vec<String>, AudioError> {
    let mut all = Vec::new();
    for host_id in cpal::available_hosts() {
        if let Ok(host) = cpal::host_from_id(host_id) {
            if let Ok(devices) = host.output_devices() {
                for dev in devices {
                    if let Ok(name) = dev.name() {
                        all.push(name);
                    }
                }
            }
        }
    }
    all.sort();
    all.dedup();
    Ok(all)
}

/// Returns the primary local IP address of the machine.
#[tauri::command]
pub fn get_local_ip() -> Result<String, AudioError> {
    use local_ip_address::local_ip;
    match local_ip() {
        Ok(ip) => Ok(ip.to_string()),
        Err(e) => Err(AudioError::IpDetectionFailed(e.to_string())),
    }
}

#[tauri::command]
pub fn get_stream_state() -> super::session::SessionStatus {
    super::session::SESSION.snapshot()
}

async fn request(
    state: &AudioState,
    command: AudioCommand,
    rx: mpsc::Receiver<Result<SessionStatus, String>>,
) -> Result<SessionStatus, AudioError> {
    state
        .tx
        .lock()
        .map_err(|_| AudioError::ChannelError("mutex poisoned".into()))?
        .try_send(command)
        .map_err(|e| AudioError::ChannelError(e.to_string()))?;
    tauri::async_runtime::spawn_blocking(move || {
        rx.recv()
            .map_err(|e| AudioError::ChannelError(e.to_string()))?
            .map_err(AudioError::StreamBuildError)
    })
    .await
    .map_err(|e| AudioError::ChannelError(e.to_string()))?
}
fn validate_profile(profile: &str) -> Result<(), AudioError> {
    if ["ultra-low", "balanced", "robust", "broadcast", "custom"].contains(&profile) {
        Ok(())
    } else {
        Err(AudioError::NetworkError("Unknown latency profile".into()))
    }
}
#[allow(clippy::too_many_arguments)]
fn validate_source(
    device: &str,
    ip: &str,
    port: u16,
    rate: u32,
    transport: &str,
    server: bool,
    profile: &str,
    allowlist: &str,
    chunk: u32,
    min: u32,
    max: u32,
) -> Result<(), AudioError> {
    validate_profile(profile)?;
    if device.trim().is_empty() || port == 0 || !(8000..=192000).contains(&rate) {
        return Err(AudioError::NetworkError(
            "Invalid device, port or sample rate".into(),
        ));
    }
    if !["tcp", "udp"].contains(&transport) {
        return Err(AudioError::NetworkError("Unknown transport".into()));
    }
    if transport == "tcp" && !server && ip.trim().is_empty() {
        return Err(AudioError::NetworkError("TCP target is required".into()));
    }
    if transport == "tcp" && !server {
        super::diagnostics::validate_host(ip).map_err(AudioError::NetworkError)?;
    }
    if transport == "tcp" && server {
        super::transport::allowlist::validate_rules(allowlist).map_err(AudioError::NetworkError)?;
    }
    if profile == "custom"
        && (!(128..=4096).contains(&chunk) || min < 50 || max < min || max > 20000)
    {
        return Err(AudioError::NetworkError(
            "Invalid custom latency limits".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod loopback_gate_tests {
    use super::offer_loopback_devices;

    #[test]
    fn never_on_macos_or_linux() {
        for os in ["macos", "linux", "ios", "android"] {
            assert!(!offer_loopback_devices(os, true), "os={os} include=true");
            assert!(!offer_loopback_devices(os, false), "os={os} include=false");
        }
    }

    #[test]
    fn on_windows_only_when_enabled() {
        assert!(offer_loopback_devices("windows", true));
        assert!(!offer_loopback_devices("windows", false));
    }
}
