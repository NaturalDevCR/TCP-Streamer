#![deny(clippy::all)]

use super::constants::*;
use super::stats::{emit_log, StreamStats};
use cpal::traits::StreamTrait;
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::AppHandle;

pub enum AudioCommand {
    Start {
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
        app_handle: Box<AppHandle>,
    },
    Stop,
    StartSink {
        output_device: String,
        source_addr: String,
        latency_profile: String,
        app_handle: Box<AppHandle>,
    },
}

pub struct AudioState {
    pub tx: Mutex<mpsc::SyncSender<AudioCommand>>,
}

impl AudioState {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel(COMMAND_CHANNEL_CAPACITY);

        thread::spawn(move || {
            let mut current_stream_handle: Option<(cpal::Stream, StreamStats)> = None;

            loop {
                match rx.try_recv() {
                    Ok(AudioCommand::Start {
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
                        app_handle,
                    }) => {
                        if let Some((stream, stats)) = current_stream_handle.take() {
                            stats.is_running.store(false, Ordering::Relaxed);
                            stream.pause().ok();
                            drop(stream);
                            drop(stats);
                        }

                        if is_server {
                            emit_log(&app_handle, "info", format!("Starting TCP Server on port {}", port));
                        } else {
                            emit_log(&app_handle, "info", format!("Starting TCP stream to {}:{}", ip, port));
                        }

                        match super::engine::run(
                            device_name, ip, port, sample_rate, buffer_size,
                            ring_buffer_duration_ms, high_priority, dscp_strategy,
                            format, chunk_size, is_loopback, is_server, auto_reconnect,
                            enable_adaptive_buffer, min_buffer_ms, max_buffer_ms,
                            latency_profile, allowlist, transport,
                            (*app_handle).clone(),
                        ) {
                            Ok((stream, stats)) => {
                                if let Err(e) = stream.play() {
                                    emit_log(&app_handle, "error", format!("Failed to play stream: {}", e));
                                } else {
                                    current_stream_handle = Some((stream, stats));
                                    emit_log(&app_handle, "success", "Stream started successfully".to_string());
                                }
                            }
                            Err(e) => {
                                emit_log(&app_handle, "error", format!("Failed to start stream: {}", e));
                            }
                        }
                    }
                    Ok(AudioCommand::Stop) => {
                        if let Some((stream, stats)) = current_stream_handle.take() {
                            stats.is_running.store(false, Ordering::Relaxed);
                            stream.pause().ok();
                            drop(stream);
                        }
                        current_stream_handle = None;
                    }
                    Ok(AudioCommand::StartSink {
                        output_device,
                        source_addr,
                        latency_profile,
                        app_handle,
                    }) => {
                        if let Some((stream, stats)) = current_stream_handle.take() {
                            stats.is_running.store(false, Ordering::Relaxed);
                            stream.pause().ok();
                            drop(stream);
                            drop(stats);
                        }

                        emit_log(&app_handle, "info", format!("Starting Sink: subscribing to {} for output {}", source_addr, output_device));

                        match super::engine::sink::run_sink(
                            output_device, source_addr, latency_profile,
                            (*app_handle).clone(),
                        ) {
                            Ok((stream, stats)) => {
                                if let Err(e) = stream.play() {
                                    emit_log(&app_handle, "error", format!("Failed to play stream: {}", e));
                                } else {
                                    current_stream_handle = Some((stream, stats));
                                    emit_log(&app_handle, "success", "Sink started successfully".to_string());
                                }
                            }
                            Err(e) => {
                                emit_log(&app_handle, "error", format!("Failed to start sink: {}", e));
                            }
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        thread::sleep(Duration::from_millis(200));
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        break;
                    }
                }
            }
        });

        Self { tx: Mutex::new(tx) }
    }

    pub fn shutdown(&self) {
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(AudioCommand::Stop);
        }
    }
}
