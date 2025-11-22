use chrono::Local;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::HeapRb;
use serde::Serialize;
use std::io::Write;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

// Audio Packet Header (16 bytes)
// Magic: "TCP\0" (4 bytes)
// Sequence: u32 (4 bytes)
// Timestamp: u64 (8 bytes) - Microseconds
// Data Length: u32 (4 bytes)
// Total Header: 20 bytes (Wait, let's stick to a simple format first)

// Let's use a simple struct for the packet header to send over TCP
#[repr(C, packed)]
struct PacketHeader {
    magic: [u8; 4],
    sequence: u32,
    timestamp: u64,
    data_len: u32,
}

impl PacketHeader {
    fn new(sequence: u32, data_len: u32) -> Self {
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        let timestamp = since_the_epoch.as_micros() as u64;

        Self {
            magic: *b"TCP\0",
            sequence,
            timestamp,
            data_len,
        }
    }

    fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                (self as *const PacketHeader) as *const u8,
                std::mem::size_of::<PacketHeader>(),
            )
        }
    }
}

// Log Event
#[derive(Clone, Serialize)]
struct LogEvent {
    timestamp: String,
    level: String,
    message: String,
}

// Health Event
#[derive(Clone, Serialize)]
struct HealthEvent {
    buffer_usage: f32,    // 0.0 to 1.0
    network_latency: u32, // ms (estimated based on write time)
    dropped_packets: u64,
}

// Helper function to emit log events
fn emit_log(app: &AppHandle, level: &str, message: String) {
    let log = LogEvent {
        timestamp: Local::now().format("%H:%M:%S").to_string(),
        level: level.to_string(),
        message,
    };
    let _ = app.emit("log-event", log);
}

// Statistics tracker
struct StreamStats {
    #[allow(dead_code)]
    bytes_sent: Arc<AtomicU64>,
    #[allow(dead_code)]
    start_time: Instant,
    is_running: Arc<AtomicBool>,
}

impl Drop for StreamStats {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
    }
}

enum AudioCommand {
    Start {
        device_name: String,
        ip: String,
        port: u16,
        sample_rate: u32,
        buffer_size: u32,
        auto_reconnect: bool,
        app_handle: AppHandle,
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
            let mut _current_stream_handle: Option<(cpal::Stream, StreamStats)> = None;
            let mut _reconnect_handle: Option<thread::JoinHandle<()>> = None;
            let should_reconnect = Arc::new(AtomicBool::new(false));

            // Keep track of current params for reconnection
            let mut _current_params: Option<(String, String, u16, u32, u32, AppHandle)> = None;

            for command in rx {
                match command {
                    AudioCommand::Start {
                        device_name,
                        ip,
                        port,
                        sample_rate,
                        buffer_size,
                        auto_reconnect,
                        app_handle,
                    } => {
                        // Stop existing stream if any
                        _current_stream_handle = None;

                        // Store params for reconnection
                        _current_params = Some((
                            device_name.clone(),
                            ip.clone(),
                            port,
                            sample_rate,
                            buffer_size,
                            app_handle.clone(),
                        ));
                        should_reconnect.store(auto_reconnect, Ordering::Relaxed);

                        emit_log(
                            &app_handle,
                            "info",
                            format!("Starting stream to {}:{}", ip, port),
                        );

                        match start_audio_stream(
                            device_name,
                            ip,
                            port,
                            sample_rate,
                            buffer_size,
                            app_handle.clone(),
                        ) {
                            Ok((stream, stats)) => {
                                stream.play().unwrap();
                                _current_stream_handle = Some((stream, stats));
                                emit_log(
                                    &app_handle,
                                    "success",
                                    "Stream started successfully".to_string(),
                                );
                            }
                            Err(e) => {
                                emit_log(
                                    &app_handle,
                                    "error",
                                    format!("Failed to start stream: {}", e),
                                );
                                // Trigger reconnect logic if enabled
                                if auto_reconnect {
                                    // This would need more complex logic to retry,
                                    // for now we just log error.
                                    // In a real implementation, we'd loop here.
                                }
                            }
                        }
                    }
                    AudioCommand::Stop => {
                        should_reconnect.store(false, Ordering::Relaxed);
                        _current_stream_handle = None;
                        _current_params = None;
                    }
                }
            }
        });

        Self { tx: Mutex::new(tx) }
    }
}

fn start_audio_stream(
    device_name: String,
    ip: String,
    port: u16,
    sample_rate: u32,
    buffer_size: u32,
    app_handle: AppHandle,
) -> Result<(cpal::Stream, StreamStats), String> {
    let host = cpal::default_host();
    let device = if device_name == "default" {
        host.default_input_device()
    } else {
        host.input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().map(|n| n == device_name).unwrap_or(false))
    }
    .ok_or("Device not found")?;

    let config = cpal::StreamConfig {
        channels: 2,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Fixed(buffer_size),
    };

    // 1. Setup Ring Buffer
    // Size = 2 seconds of audio (48000 * 2 channels * 2 seconds = 192000 samples)
    let ring_buffer_size = (sample_rate as usize) * 2 * 2;
    let rb = HeapRb::<i16>::new(ring_buffer_size);
    let (mut prod, mut cons) = rb.split();

    // Shared stats
    let bytes_sent = Arc::new(AtomicU64::new(0));
    let is_running = Arc::new(AtomicBool::new(true));

    let bytes_sent_clone = bytes_sent.clone();
    let is_running_clone = is_running.clone();
    let app_handle_net = app_handle.clone();
    let ip_clone = ip.clone();

    // 2. Spawn Network Thread (Consumer)
    thread::spawn(move || {
        let addr = format!("{}:{}", ip_clone, port);
        let mut stream = match TcpStream::connect(&addr) {
            Ok(s) => s,
            Err(e) => {
                emit_log(
                    &app_handle_net,
                    "error",
                    format!("TCP Connect failed: {}", e),
                );
                return;
            }
        };

        if let Err(e) = stream.set_nodelay(true) {
            emit_log(
                &app_handle_net,
                "warn",
                format!("Failed to set nodelay: {}", e),
            );
        }

        let mut sequence: u32 = 0;
        let mut temp_buffer = [0i16; 1024]; // Read in chunks
        let mut dropped_packets: u64 = 0;

        emit_log(
            &app_handle_net,
            "info",
            "Network thread started".to_string(),
        );

        while is_running_clone.load(Ordering::Relaxed) {
            // Check buffer usage for health monitoring
            let occupied = cons.len();
            let capacity = cons.capacity();
            let usage = occupied as f32 / capacity as f32;

            if usage > 0.9 {
                // Buffer near full! Network is too slow.
                dropped_packets += 1;
                // We could skip sending to catch up, or just log it.
            }

            // Emit health event occasionally (e.g., every 100 packets)
            if sequence % 100 == 0 {
                let _ = app_handle_net.emit(
                    "health-event",
                    HealthEvent {
                        buffer_usage: usage,
                        network_latency: 0, // TODO: Measure write time
                        dropped_packets,
                    },
                );
            }

            // Read from Ring Buffer
            // We read as much as available, up to temp_buffer size
            let count = cons.pop_slice(&mut temp_buffer);

            if count > 0 {
                let start_write = Instant::now();

                // Prepare Packet
                // 1. Header
                let data_len = (count * 2) as u32; // 2 bytes per sample
                let header = PacketHeader::new(sequence, data_len);

                // 2. Payload (Convert i16 to bytes)
                let mut payload = Vec::with_capacity(data_len as usize);
                for i in 0..count {
                    payload.extend_from_slice(&temp_buffer[i].to_le_bytes());
                }

                // 3. Send Header + Payload
                if let Err(e) = stream.write_all(header.as_bytes()) {
                    emit_log(&app_handle_net, "error", format!("Write error: {}", e));
                    break;
                }
                if let Err(e) = stream.write_all(&payload) {
                    emit_log(&app_handle_net, "error", format!("Write error: {}", e));
                    break;
                }

                let _write_time = start_write.elapsed().as_millis();
                // Update stats
                bytes_sent_clone.fetch_add(
                    (header.as_bytes().len() + payload.len()) as u64,
                    Ordering::Relaxed,
                );
                sequence = sequence.wrapping_add(1);
            } else {
                // Buffer empty, sleep briefly to avoid busy loop
                thread::sleep(Duration::from_millis(1));
            }
        }
        emit_log(
            &app_handle_net,
            "info",
            "Network thread stopped".to_string(),
        );
    });

    // 3. Setup Audio Stream (Producer)
    let app_handle_err = app_handle.clone();
    let err_fn = move |err| {
        emit_log(&app_handle_err, "error", format!("Stream error: {}", err));
    };

    // RMS Silence Detection State
    let silence_threshold = 50.0; // Adjust as needed

    let audio_stream = device
        .build_input_stream(
            &config,
            move |data: &[i16], _: &_| {
                // 1. Silence Detection
                // Calculate RMS of this chunk
                let mut sum_squares = 0.0;
                for &sample in data {
                    sum_squares += (sample as f32) * (sample as f32);
                }
                let rms = (sum_squares / data.len() as f32).sqrt();

                if rms > silence_threshold {
                    // 2. Push to Ring Buffer
                    // This is non-blocking (unless buffer is full, but HeapRb handles it)
                    // If full, it returns number of elements pushed (could be 0)
                    let pushed = prod.push_slice(data);
                    if pushed < data.len() {
                        // Buffer overflow! Producer is too fast for Consumer (Network)
                        // This means XRUN (Overrun)
                        // We just drop the rest of the samples
                    }
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| e.to_string())?;

    Ok((
        audio_stream,
        StreamStats {
            bytes_sent,
            start_time: Instant::now(),
            is_running,
        },
    ))
}

#[tauri::command]
pub fn get_input_devices() -> Result<Vec<String>, String> {
    let host = cpal::default_host();
    let devices = host.input_devices().map_err(|e| e.to_string())?;
    Ok(devices.map(|d| d.name().unwrap_or_default()).collect())
}

#[tauri::command]
pub async fn start_stream(
    state: tauri::State<'_, AudioState>,
    device_name: String,
    ip: String,
    port: u16,
    sample_rate: u32,
    buffer_size: u32,
    auto_reconnect: bool,
    app_handle: AppHandle,
) -> Result<(), String> {
    let tx = state.tx.lock().map_err(|e| e.to_string())?;
    tx.send(AudioCommand::Start {
        device_name,
        ip,
        port,
        sample_rate,
        buffer_size,
        auto_reconnect,
        app_handle,
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn stop_stream(state: tauri::State<'_, AudioState>) -> Result<(), String> {
    let tx = state.tx.lock().map_err(|e| e.to_string())?;
    tx.send(AudioCommand::Stop).map_err(|e| e.to_string())?;
    Ok(())
}
