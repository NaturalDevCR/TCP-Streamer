#![deny(clippy::all)]

use super::constants::*;
use super::session::{publish, SessionStatus, SESSION};
use super::stats::{emit_log, StreamStats};
use cpal::traits::StreamTrait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::AppHandle;
type Reply = mpsc::Sender<Result<SessionStatus, String>>;

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
        fixed_latency_ms: u32,
        latency_profile: String,
        allowlist: String,
        transport: String,
        psk: String,
        app_handle: Box<AppHandle>,
        reply: Reply,
        running: Arc<AtomicBool>,
    },
    Stop {
        reply: Option<Reply>,
        app_handle: Option<Box<AppHandle>>,
    },
    StartSink {
        output_device: String,
        source_addr: String,
        latency_profile: String,
        overrides: super::engine::latency::LatencyOverrides,
        psk: String,
        app_handle: Box<AppHandle>,
        reply: Reply,
        running: Arc<AtomicBool>,
    },
}

pub struct AudioState {
    pub tx: Mutex<mpsc::SyncSender<AudioCommand>>,
    cancellation: Mutex<Option<Arc<AtomicBool>>>,
}

impl AudioState {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel(COMMAND_CHANNEL_CAPACITY);

        thread::spawn(move || {
            let mut current_stream_handle: Option<(cpal::Stream, StreamStats)> = None;
            let mut current_app: Option<AppHandle> = None;

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
                        fixed_latency_ms,
                        latency_profile,
                        allowlist,
                        transport,
                        psk,
                        app_handle,
                        reply,
                        running,
                    }) => {
                        if let Some((stream, stats)) = current_stream_handle.take() {
                            stats.is_running.store(false, Ordering::Relaxed);
                            stream.pause().ok();
                            drop(stream);
                            drop(stats);
                        }

                        let status = SESSION.begin();
                        let session_id = status.session_id;
                        publish(&app_handle, session_id, "starting", "Starting audio source");
                        current_app = Some((*app_handle).clone());
                        if transport == "udp" {
                            emit_log(
                                &app_handle,
                                "info",
                                format!("Starting Native UDP source on port {port}"),
                            );
                        } else if is_server {
                            emit_log(
                                &app_handle,
                                "info",
                                format!("Starting TCP Server on port {}", port),
                            );
                        } else {
                            emit_log(
                                &app_handle,
                                "info",
                                format!("Starting TCP stream to {}:{}", ip, port),
                            );
                        }

                        match super::engine::run(
                            device_name,
                            ip,
                            port,
                            sample_rate,
                            buffer_size,
                            ring_buffer_duration_ms,
                            high_priority,
                            dscp_strategy,
                            format,
                            chunk_size,
                            is_loopback,
                            is_server,
                            auto_reconnect,
                            enable_adaptive_buffer,
                            min_buffer_ms,
                            max_buffer_ms,
                            fixed_latency_ms,
                            latency_profile,
                            allowlist,
                            transport,
                            psk,
                            (*app_handle).clone(),
                            session_id,
                            running.clone(),
                        ) {
                            Ok((stream, stats)) => {
                                if !running.load(Ordering::Relaxed) {
                                    drop(stream);
                                    drop(stats);
                                    publish(&app_handle, session_id, "stopped", "Start cancelled");
                                    let _ = reply.send(Ok(SESSION.snapshot()));
                                } else if let Err(e) = stream.play() {
                                    publish(
                                        &app_handle,
                                        session_id,
                                        "error",
                                        format!("Playback failed: {e}"),
                                    );
                                    let _ = reply.send(Err(e.to_string()));
                                    emit_log(
                                        &app_handle,
                                        "error",
                                        format!("Failed to play stream: {}", e),
                                    );
                                } else {
                                    current_stream_handle = Some((stream, stats));
                                    if SESSION.snapshot().state == "starting" {
                                        publish(
                                            &app_handle,
                                            session_id,
                                            "waiting",
                                            "Waiting for receiver",
                                        );
                                    }
                                    let _ = reply.send(Ok(SESSION.snapshot()));
                                    emit_log(
                                        &app_handle,
                                        "success",
                                        "Stream started successfully".to_string(),
                                    );
                                }
                            }
                            Err(e) => {
                                publish(
                                    &app_handle,
                                    session_id,
                                    if running.load(Ordering::Relaxed) {
                                        "error"
                                    } else {
                                        "stopped"
                                    },
                                    e.clone(),
                                );
                                let _ = reply.send(Err(e.clone()));
                                emit_log(
                                    &app_handle,
                                    "error",
                                    format!("Failed to start stream: {}", e),
                                );
                            }
                        }
                    }
                    Ok(AudioCommand::Stop { reply, app_handle }) => {
                        if let Some((stream, stats)) = current_stream_handle.take() {
                            stats.is_running.store(false, Ordering::Relaxed);
                            stream.pause().ok();
                            drop(stream);
                        }
                        current_stream_handle = None;
                        if let Some(app) = app_handle.as_deref().or(current_app.as_ref()) {
                            publish(app, SESSION.snapshot().session_id, "stopped", "Ready");
                        }
                        if let Some(reply) = reply {
                            let _ = reply.send(Ok(SESSION.snapshot()));
                        }
                    }
                    Ok(AudioCommand::StartSink {
                        output_device,
                        source_addr,
                        latency_profile,
                        overrides,
                        psk,
                        app_handle,
                        reply,
                        running,
                    }) => {
                        if let Some((stream, stats)) = current_stream_handle.take() {
                            stats.is_running.store(false, Ordering::Relaxed);
                            stream.pause().ok();
                            drop(stream);
                            drop(stats);
                        }

                        let status = SESSION.begin();
                        let session_id = status.session_id;
                        publish(&app_handle, session_id, "starting", "Subscribing to source");
                        current_app = Some((*app_handle).clone());
                        emit_log(
                            &app_handle,
                            "info",
                            format!(
                                "Starting Sink: subscribing to {} for output {}",
                                source_addr, output_device
                            ),
                        );

                        match super::engine::sink::run_sink(
                            output_device,
                            source_addr,
                            latency_profile,
                            overrides,
                            psk,
                            (*app_handle).clone(),
                            session_id,
                            running.clone(),
                        ) {
                            Ok((stream, stats)) => {
                                if !running.load(Ordering::Relaxed) {
                                    drop(stream);
                                    drop(stats);
                                    publish(&app_handle, session_id, "stopped", "Start cancelled");
                                    let _ = reply.send(Ok(SESSION.snapshot()));
                                } else if let Err(e) = stream.play() {
                                    publish(
                                        &app_handle,
                                        session_id,
                                        "error",
                                        format!("Playback failed: {e}"),
                                    );
                                    let _ = reply.send(Err(e.to_string()));
                                    emit_log(
                                        &app_handle,
                                        "error",
                                        format!("Failed to play stream: {}", e),
                                    );
                                } else {
                                    current_stream_handle = Some((stream, stats));
                                    if SESSION.snapshot().state == "starting" {
                                        publish(
                                            &app_handle,
                                            session_id,
                                            "waiting",
                                            "Waiting for source audio",
                                        );
                                    }
                                    let _ = reply.send(Ok(SESSION.snapshot()));
                                    emit_log(
                                        &app_handle,
                                        "success",
                                        "Sink started successfully".to_string(),
                                    );
                                }
                            }
                            Err(e) => {
                                publish(
                                    &app_handle,
                                    session_id,
                                    if running.load(Ordering::Relaxed) {
                                        "error"
                                    } else {
                                        "stopped"
                                    },
                                    e.clone(),
                                );
                                let _ = reply.send(Err(e.clone()));
                                emit_log(
                                    &app_handle,
                                    "error",
                                    format!("Failed to start sink: {}", e),
                                );
                            }
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        if current_stream_handle
                            .as_ref()
                            .is_some_and(|(_, stats)| !stats.is_running.load(Ordering::Relaxed))
                        {
                            if let Some((stream, stats)) = current_stream_handle.take() {
                                let _ = stream.pause();
                                drop(stream);
                                drop(stats);
                            }
                            if SESSION.snapshot().state != "error" {
                                if let Some(app) = &current_app {
                                    publish(
                                        app,
                                        SESSION.snapshot().session_id,
                                        "error",
                                        "Audio transport stopped unexpectedly",
                                    );
                                }
                            }
                        }
                        thread::sleep(Duration::from_millis(20));
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        break;
                    }
                }
            }
        });

        Self {
            tx: Mutex::new(tx),
            cancellation: Mutex::new(None),
        }
    }

    pub fn new_running_flag(&self) -> Arc<AtomicBool> {
        let running = Arc::new(AtomicBool::new(true));
        let mut guard = self.cancellation.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(old) = guard.replace(running.clone()) {
            old.store(false, Ordering::Relaxed);
        }
        running
    }
    pub fn cancel(&self) {
        if let Some(running) = self
            .cancellation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            running.store(false, Ordering::Relaxed);
        }
    }
    pub fn shutdown(&self) {
        self.cancel();
        let (reply, ack) = mpsc::channel();
        if let Ok(tx) = self.tx.lock() {
            if tx
                .try_send(AudioCommand::Stop {
                    reply: Some(reply),
                    app_handle: None,
                })
                .is_ok()
            {
                let _ = ack.recv_timeout(Duration::from_secs(2));
            }
        }
    }
}
