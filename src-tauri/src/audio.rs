use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use tauri::State;

enum AudioCommand {
    Start {
        device_name: String,
        ip: String,
        port: u16,
        sample_rate: u32,
        buffer_size: u32,
    },
    Stop,
}

pub struct AudioState {
    tx: Mutex<mpsc::Sender<AudioCommand>>,
}

impl AudioState {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut _current_stream: Option<cpal::Stream> = None;

            while let Ok(cmd) = rx.recv() {
                match cmd {
                    AudioCommand::Start {
                        device_name,
                        ip,
                        port,
                        sample_rate,
                        buffer_size,
                    } => {
                        // Stop existing stream if any
                        _current_stream = None;

                        match start_audio_stream(device_name, ip, port, sample_rate, buffer_size) {
                            Ok(stream) => {
                                _current_stream = Some(stream);
                            }
                            Err(e) => {
                                eprintln!("Failed to start stream: {}", e);
                            }
                        }
                    }
                    AudioCommand::Stop => {
                        _current_stream = None;
                    }
                }
            }
        });

        Self { tx: Mutex::new(tx) }
    }
}

#[derive(serde::Serialize, Clone)]
pub struct AudioDevice {
    pub name: String,
}

#[tauri::command]
pub fn get_input_devices() -> Result<Vec<AudioDevice>, String> {
    println!("Fetching input devices...");
    let host = cpal::default_host();

    // Standard input devices
    let input_devices = host.input_devices().map_err(|e| e.to_string())?;
    let mut result = Vec::new();

    for device in input_devices {
        if let Ok(name) = device.name() {
            println!("Found input device: {}", name);
            result.push(AudioDevice { name });
        }
    }

    // Windows WASAPI Loopback: Add output devices as "Loopback: Name"
    #[cfg(target_os = "windows")]
    {
        if host.id() == cpal::HostId::Wasapi {
            if let Ok(output_devices) = host.output_devices() {
                for device in output_devices {
                    if let Ok(name) = device.name() {
                        let loopback_name = format!("Loopback: {}", name);
                        println!("Found loopback candidate: {}", loopback_name);
                        result.push(AudioDevice {
                            name: loopback_name,
                        });
                    }
                }
            }
        }
    }

    println!("Returning {} devices", result.len());
    Ok(result)
}

fn start_audio_stream(
    device_name: String,
    ip: String,
    port: u16,
    sample_rate: u32,
    buffer_size: u32,
) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let mut device = None;
    let mut is_loopback = false;

    // Check for Loopback prefix (Windows WASAPI)
    if device_name.starts_with("Loopback: ") {
        let _real_name = device_name.trim_start_matches("Loopback: ");
        is_loopback = true;

        #[cfg(target_os = "windows")]
        if let Ok(devices) = host.output_devices() {
            for d in devices {
                if let Ok(name) = d.name() {
                    if name == _real_name {
                        device = Some(d);
                        break;
                    }
                }
            }
        }
    } else {
        // Standard input device search
        if device_name == "default" {
            device = host.default_input_device();
        } else {
            let devices = host.input_devices().map_err(|e| e.to_string())?;
            for d in devices {
                if let Ok(name) = d.name() {
                    if name == device_name {
                        device = Some(d);
                        break;
                    }
                }
            }
        }
    }

    let device = device.ok_or("Device not found")?;
    println!(
        "Using device: {} (Loopback: {})",
        device.name().unwrap_or_default(),
        is_loopback
    );

    let config = cpal::StreamConfig {
        channels: 2,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Fixed(buffer_size),
    };

    let addr = format!("{}:{}", ip, port);
    println!("Connecting to {}", addr);

    // Connect to TCP server
    let mut stream = TcpStream::connect(&addr).map_err(|e| format!("Failed to connect: {}", e))?;
    stream.set_nodelay(true).map_err(|e| e.to_string())?;

    let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

    let stream = device
        .build_input_stream(
            &config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                // Convert to bytes and send
                // Note: This is blocking the audio thread, which is not ideal but simple for now.
                // For production, use a ring buffer.
                let mut bytes = Vec::with_capacity(data.len() * 2);
                for &sample in data {
                    bytes.extend_from_slice(&sample.to_le_bytes());
                }

                if let Err(e) = stream.write_all(&bytes) {
                    eprintln!("Failed to write to TCP: {}", e);
                }
            },
            err_fn,
            None, // Timeout
        )
        .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

#[tauri::command]
pub fn start_stream(
    state: State<'_, AudioState>,
    device_name: String,
    ip: String,
    port: u16,
    sample_rate: u32,
    buffer_size: u32,
) -> Result<(), String> {
    let tx = state.tx.lock().map_err(|e| e.to_string())?;
    tx.send(AudioCommand::Start {
        device_name,
        ip,
        port,
        sample_rate,
        buffer_size,
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn stop_stream(state: State<'_, AudioState>) -> Result<(), String> {
    let tx = state.tx.lock().map_err(|e| e.to_string())?;
    tx.send(AudioCommand::Stop).map_err(|e| e.to_string())?;
    Ok(())
}
