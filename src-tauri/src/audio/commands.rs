#![deny(clippy::all)]

use super::error::AudioError;
use super::manager::{AudioCommand, AudioState};
use cpal::traits::{DeviceTrait, HostTrait};
use log::debug;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, State};

static DEVICE_CACHE: Mutex<Option<(Vec<String>, Instant)>> = Mutex::new(None);
const DEVICE_CACHE_TTL_SECS: u64 = 30;

/// Starts streaming audio from the given device to the specified IP:port.
///
/// Supports both client mode (connects to a remote server) and server mode
/// (listens for incoming connections). Audio is captured via CPAL and sent
/// over TCP as raw PCM or WAV.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn start_stream(
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
    latency_profile: String,
    allowlist: String,
    transport: String,
    psk: String,
) -> Result<(), AudioError> {
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
        latency_profile,
        allowlist,
        transport,
        psk,
        app_handle: Box::new(app_handle),
    };

    state
        .tx
        .lock()
        .map_err(|_| AudioError::ChannelError("mutex poisoned".to_string()))?
        .send(command)
        .map_err(|e| AudioError::ChannelError(e.to_string()))
}

/// Stops the currently active audio stream.
#[tauri::command]
pub fn stop_stream(state: State<'_, AudioState>) -> Result<(), AudioError> {
    state
        .tx
        .lock()
        .map_err(|_| AudioError::ChannelError("mutex poisoned".to_string()))?
        .send(AudioCommand::Stop)
        .map_err(|e| AudioError::ChannelError(e.to_string()))
}

/// Starts the sink: subscribes to a native UDP source and plays audio
/// to the selected output device.
#[tauri::command]
pub fn start_sink(
    state: State<'_, AudioState>,
    app_handle: AppHandle,
    output_device: String,
    source_addr: String,
    latency_profile: String,
    psk: String,
) -> Result<(), AudioError> {
    state.tx.lock()
        .map_err(|_| AudioError::ChannelError("mutex poisoned".to_string()))?
        .send(AudioCommand::StartSink {
            output_device, source_addr, latency_profile, psk, app_handle: Box::new(app_handle),
        })
        .map_err(|e| AudioError::ChannelError(e.to_string()))
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
pub fn get_input_devices(
    #[allow(unused_variables)] include_loopback: bool,
) -> Result<Vec<String>, AudioError> {
    // Check cache
    if let Ok(cache) = DEVICE_CACHE.lock() {
        if let Some((cached_devices, timestamp)) = cache.as_ref() {
            if timestamp.elapsed().as_secs() < DEVICE_CACHE_TTL_SECS {
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

                // 2. WASAPI Loopback (Windows-specific, usually)
                if let Ok(output_devices) = host.output_devices() {
                    for dev in output_devices {
                        if let Ok(name) = dev.name() {
                            all_devices.push(format!("[Loopback] {}", name));
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
        *cache = Some((all_devices.clone(), Instant::now()));
    }

    Ok(all_devices)
}

// Stopping the sink reuses `stop_stream` (sends Stop to the manager,
// which stops whatever is currently running).

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
