use super::manager::{AudioCommand, AudioState};
use cpal::traits::{DeviceTrait, HostTrait};
use log::debug;
use tauri::{AppHandle, State};

#[tauri::command]
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
) -> Result<(), String> {
    // buffer_size accepted from frontend for backward compatibility but not used internally
    let _ = buffer_size;
    let command = AudioCommand::Start {
        device_name,
        ip,
        port,
        sample_rate,
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
        app_handle,
    };

    state
        .tx
        .lock()
        .map_err(|e| e.to_string())?
        .send(command)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop_stream(state: State<'_, AudioState>) -> Result<(), String> {
    state
        .tx
        .lock()
        .map_err(|e| e.to_string())?
        .send(AudioCommand::Stop)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_os_type() -> String {
    std::env::consts::OS.to_string()
}

#[tauri::command]
pub fn get_input_devices(
    #[allow(unused_variables)] include_loopback: bool,
) -> Result<Vec<String>, String> {
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

    Ok(all_devices)
}

#[tauri::command]
pub fn get_local_ip() -> Result<String, String> {
    use local_ip_address::local_ip;
    match local_ip() {
        Ok(ip) => Ok(ip.to_string()),
        Err(e) => Err(format!("Failed to get local IP: {}", e)),
    }
}
