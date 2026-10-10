//! UDP sink I/O: subscribe to a source, receive audio, de-jitter, decode, and
//! feed the playback ring buffer.

use super::{
    jitter::{JitterBuffer, Pop},
    packet,
};
use crate::audio::engine::decoder::decode_pcm_i16_le_to_f32;
use ringbuf::HeapProducer;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Cadence of the continuous drift correction. The controller is pure and
/// takes no clock of its own, so this is both the caller's tick interval and
/// the timebase its gains are derived against — they must not diverge.
const TRIM_TICK: Duration = Duration::from_millis(100);

/// Result of the subscribe handshake.
pub struct Subscribed {
    pub socket: UdpSocket,
    pub info: packet::StreamInfo,
    psk: String,
    active: bool,
}

impl Subscribed {
    /// Transfer cleanup responsibility once the receiver worker has started.
    pub fn disarm(&mut self) {
        self.active = false;
    }
    /// Transfer the lease to receive_loop after device construction succeeds.
    #[allow(dead_code)] // Public/test convenience API; orchestrator disarms after spawning.
    pub fn into_parts(mut self) -> std::io::Result<(UdpSocket, packet::StreamInfo)> {
        let socket = self.socket.try_clone()?;
        self.active = false;
        Ok((socket, self.info))
    }
}

impl Drop for Subscribed {
    fn drop(&mut self) {
        if self.active {
            let mut goodbye = Vec::new();
            packet::encode_unsubscribe(&self.info, &self.psk, &mut goodbye);
            let _ = self.socket.send(&goodbye);
        }
    }
}

struct AttemptCleanup<'a> {
    socket: &'a UdpSocket,
    salt_b: u64,
    psk: &'a str,
    active: bool,
    info: Option<packet::StreamInfo>,
}

impl Drop for AttemptCleanup<'_> {
    fn drop(&mut self) {
        if self.active {
            let mut cancel = Vec::new();
            if let Some(info) = self.info {
                packet::encode_unsubscribe(&info, self.psk, &mut cancel);
                let _ = self.socket.send(&cancel);
            }
            packet::encode_cancel_subscribe(self.salt_b, self.psk, &mut cancel);
            let _ = self.socket.send(&cancel);
        }
    }
}

/// Sends SUBSCRIBE to `source_addr` and waits for STREAM_INFO. Returns the bound
/// socket (connected to the source) and the negotiated format.
#[allow(dead_code)] // compatibility wrapper; application startup uses the cancellable variant
pub fn subscribe(
    source_addr: &str,
    salt_b: u64,
    psk: &str,
    timeout: Duration,
) -> std::io::Result<Subscribed> {
    subscribe_cancellable(source_addr, salt_b, psk, timeout, &AtomicBool::new(true))
}

/// Handshake with bounded cancellation latency for a manual Stop during startup.
pub fn subscribe_cancellable(
    source_addr: &str,
    salt_b: u64,
    psk: &str,
    timeout: Duration,
    running: &AtomicBool,
) -> std::io::Result<Subscribed> {
    if !running.load(Ordering::Acquire) {
        return Err(cancelled());
    }
    if timeout.is_zero() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "subscribe timeout must be positive",
        ));
    }
    let addr = super::super::resolve::resolve_endpoint_cancellable(source_addr, running, timeout)?
        .into_iter()
        .next()
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "No source addresses")
        })?;
    let socket = UdpSocket::bind(if addr.is_ipv6() {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    })?;
    socket.connect(addr)?;
    socket.set_read_timeout(Some(timeout.min(Duration::from_millis(50))))?;
    let mut cleanup = AttemptCleanup {
        socket: &socket,
        salt_b,
        psk,
        active: true,
        info: None,
    };

    let mut sub = Vec::new();
    packet::encode_authenticated_subscribe(salt_b, psk, &mut sub);

    let mut buf = [0u8; 2048];
    let deadline = Instant::now() + timeout * 3;
    loop {
        if !running.load(Ordering::Acquire) {
            return Err(cancelled());
        }
        socket.send(&sub)?;
        match socket.recv(&mut buf) {
            Ok(n) => {
                if let Some(info) = packet::decode_authenticated_info(&buf[..n], salt_b, psk) {
                    cleanup.info = Some(info);
                    if !running.load(Ordering::Acquire) {
                        return Err(cancelled());
                    }
                    let mut confirm = Vec::new();
                    packet::encode_confirm(&info, psk, &mut confirm);
                    socket.send(&confirm)?;
                    cleanup.active = false;
                    drop(cleanup);
                    let subscribed = Subscribed {
                        socket,
                        info,
                        psk: psk.to_string(),
                        active: true,
                    };
                    if !running.load(Ordering::Acquire) {
                        return Err(cancelled());
                    }
                    subscribed
                        .socket
                        .set_read_timeout(Some(Duration::from_millis(100)))?;
                    return Ok(subscribed);
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(e),
        }
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "No compatible authenticated UDP v2 STREAM_INFO: check address and shared key",
            ));
        }
    }
}

fn cancelled() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::Interrupted,
        "UDP subscription cancelled",
    )
}

/// Receives audio packets until `running` is cleared, converting through
/// `pipeline` (source format → device format) into `producer`. Resends
/// SUBSCRIBE every ~1s as a heartbeat.
/// `target_samples` is the target playback ring occupancy (device-unit
/// samples) for drift correction; corrections are applied in whole device
/// frames (`out_channels`) so interleaving can never shift.
/// `out_rate` is the device sample rate; with `out_channels` and the trim
/// cadence below it defines the plant the trim controller is tuned against.
#[allow(clippy::too_many_arguments)]
pub fn receive_loop(
    socket: &UdpSocket,
    mut salt_b: u64,
    lost_after: usize,
    psk: String,
    mut initial_info: packet::StreamInfo,
    target_samples: usize,
    out_rate: u32,
    out_channels: u16,
    mut pipeline: crate::audio::engine::convert::SinkPipeline,
    mut producer: HeapProducer<f32>,
    running: Arc<AtomicBool>,
    trim_ppm_out: Arc<AtomicI32>,
    metrics: Arc<crate::audio::stats::ReceiveMetrics>,
) {
    metrics.wire_format.store(
        (u64::from(initial_info.sample_rate) << 16) | u64::from(initial_info.channels),
        Ordering::Relaxed,
    );
    let ch = out_channels.max(1) as usize;
    let mut jb = JitterBuffer::new(lost_after);
    // Sized for the largest UDP datagram, not the typical one: a truncated
    // recv silently corrupts a frame (and fails AEAD outright).
    let mut buf = [0u8; 65535];
    let mut decoded: Vec<f32> = Vec::new();
    let mut converted: Vec<f32> = Vec::new();
    let mut sub = Vec::new();
    packet::encode_authenticated_subscribe(salt_b, &psk, &mut sub);
    let mut key =
        (!psk.is_empty()).then(|| super::crypto::derive_key(&psk, initial_info.salt_a, salt_b));
    let mut nonce_salt = super::crypto::nonce_salt(initial_info.salt_a, salt_b);
    let mut last_audio = Instant::now();
    let mut next_frame_pos: Option<u64> = None;
    let mut recent_sessions = std::collections::VecDeque::from([initial_info.salt_a]);
    let mut last_hb = Instant::now();
    let mut heartbeat_counter = 0u64;
    let mut replay = super::crypto::ReplayWindow::new();
    // The ASRC below absorbs ordinary clock drift continuously, so drop/insert
    // is demoted to a safety net: a margin of three quarters of the target
    // means it fires only above 1.75x or below 0.25x, which is a network stall
    // or a device glitch, not drift. Those excursions are larger than a few
    // ppm of trim can absorb in any reasonable time, and there a single
    // audible correction beats minutes of wrong latency.
    let mut drift = super::drift::DriftController::new(
        target_samples as f32,
        (target_samples * 3 / 4) as f32,
        200,
    );
    // The controller's gains are derived from the plant, so it needs the same
    // cadence the loop below ticks it at, expressed as samples of flow.
    let samples_per_tick = out_rate as f64 * ch as f64 * TRIM_TICK.as_secs_f64();
    let mut trim = super::trim::TrimController::new(target_samples as f32, samples_per_tick);
    let mut last_trim_tick = Instant::now();
    let mut skip_samples: usize = 0;
    let mut insert_silence: usize = 0;

    // Prefill the playback ring so the standing latency STARTS at its
    // configured value instead of climbing to it. Nothing else ever fills it:
    // the sink would otherwise begin at zero occupancy and the ASRC — capped at
    // MAX_TRIM_PPM — can only reposition 0.2 ms of audio per second, a quarter
    // of an hour to reach a 250 ms target. The Broadcast profile's promise is a
    // latency an operator measures once, so it has to be right from the first
    // second. Whole device frames only, so channel interleaving cannot shift —
    // clamped to the free space, because a short push of a partial frame would
    // shift it for the life of the stream.
    let prefill = target_samples.min(producer.free_len()) / ch * ch;
    if prefill > 0 {
        let silence = vec![0.0f32; prefill];
        let _ = producer.push_slice(&silence);
    }

    while running.load(Ordering::Relaxed) {
        // Let the session supervisor report/retry a prolonged source outage.
        if last_audio.elapsed() > Duration::from_secs(30) {
            break;
        }
        if last_hb.elapsed() >= Duration::from_secs(1) {
            if last_audio.elapsed() >= Duration::from_secs(3) {
                if let Ok(new_salt) = super::crypto::random_salt() {
                    salt_b = new_salt;
                    packet::encode_authenticated_subscribe(salt_b, &psk, &mut sub);
                }
            }
            let mut heartbeat = Vec::new();
            packet::encode_heartbeat(&initial_info, heartbeat_counter, &psk, &mut heartbeat);
            let _ = socket.send(&heartbeat);
            heartbeat_counter = heartbeat_counter.saturating_add(1);
            let _ = socket.send(&sub);
            last_hb = Instant::now();
        }
        match socket.recv(&mut buf) {
            Ok(n) => {
                if let Some(info) = packet::decode_authenticated_info(&buf[..n], salt_b, &psk) {
                    let mut confirm = Vec::new();
                    packet::encode_confirm(&info, &psk, &mut confirm);
                    let _ = socket.send(&confirm);
                    if info.salt_a != initial_info.salt_a && !recent_sessions.contains(&info.salt_a)
                    {
                        metrics.reconnects.fetch_add(1, Ordering::Relaxed);
                        recent_sessions.push_back(info.salt_a);
                        if recent_sessions.len() > 16 {
                            recent_sessions.pop_front();
                        }
                        key = (!psk.is_empty())
                            .then(|| super::crypto::derive_key(&psk, info.salt_a, salt_b));
                        nonce_salt = super::crypto::nonce_salt(info.salt_a, salt_b);
                        pipeline = crate::audio::engine::convert::SinkPipeline::new(
                            info.sample_rate,
                            info.channels,
                            out_rate,
                            out_channels,
                        );
                        initial_info = info;
                        metrics.wire_format.store(
                            (u64::from(info.sample_rate) << 16) | u64::from(info.channels),
                            Ordering::Relaxed,
                        );
                        heartbeat_counter = 0;
                        replay = super::crypto::ReplayWindow::new();
                        jb = JitterBuffer::new(lost_after);
                        next_frame_pos = None;
                        skip_samples = 0;
                        insert_silence = 0;
                        trim = super::trim::TrimController::new(
                            target_samples as f32,
                            samples_per_tick,
                        );
                    }
                } else if let Some((h, payload)) = packet::decode_audio(&buf[..n]) {
                    if h.session != initial_info.salt_a
                        || h.frame_count == 0
                        || h.frame_count as usize * initial_info.channels as usize * 2
                            > super::source::MAX_PAYLOAD_BYTES
                    {
                        continue;
                    }
                    let expected_bytes =
                        h.frame_count as usize * initial_info.channels as usize * 2;
                    if payload.len() != expected_bytes + if h.flags == 1 { 16 } else { 0 }
                        || h.frame_pos.checked_add(h.frame_count as u64).is_none()
                    {
                        metrics.auth_failures.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    let Some(pcm) = super::crypto::authenticated_audio(
                        key.as_ref(),
                        nonce_salt,
                        &h,
                        &buf[..packet::AUDIO_HEADER_LEN],
                        payload,
                        &mut replay,
                    ) else {
                        metrics.auth_failures.fetch_add(1, Ordering::Relaxed);
                        continue;
                    };
                    if pcm.len() != h.frame_count as usize * initial_info.channels as usize * 2 {
                        continue;
                    }
                    metrics
                        .bytes_received
                        .fetch_add(pcm.len() as u64, Ordering::Relaxed);
                    metrics.received_packets.fetch_add(1, Ordering::Relaxed);
                    last_audio = Instant::now();
                    // Keep authenticated frame metadata beside the reordered PCM.
                    let mut framed = h.frame_pos.to_be_bytes().to_vec();
                    framed.extend_from_slice(&pcm);
                    jb.push(h.seq, framed);
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => break,
        }
        // Drain the jitter buffer into the playback ring.
        loop {
            match jb.pop() {
                Pop::Frame(framed) => {
                    let frame_pos =
                        u64::from_be_bytes(framed[..8].try_into().expect("stored metadata"));
                    let pcm = &framed[8..];
                    if let Some(expected) = next_frame_pos {
                        if frame_pos < expected {
                            continue;
                        }
                        // Exact silence in source frames preserves duration even for variable datagrams.
                        // Large discontinuities rebase instead of allocating or blocking for arbitrary time.
                        let missing = frame_pos
                            .saturating_sub(expected)
                            .min(initial_info.sample_rate as u64 * 2);
                        metrics.lost_frames.fetch_add(missing, Ordering::Relaxed);
                        let mut remaining = missing;
                        while remaining > 0 {
                            let source_count = remaining.min(600) as usize;
                            decoded.clear();
                            decoded.resize(source_count * initial_info.channels as usize, 0.0);
                            pipeline.process(&decoded, &mut converted);
                            let count = converted.len().min(producer.free_len()) / ch * ch;
                            let _ = producer.push_slice(&converted[..count]);
                            remaining -= source_count as u64;
                        }
                    }
                    next_frame_pos =
                        Some(frame_pos + (pcm.len() / (initial_info.channels as usize * 2)) as u64);
                    decode_pcm_i16_le_to_f32(pcm, &mut decoded);
                    pipeline.process(&decoded, &mut converted);
                    // Apply drift correction in whole device frames.
                    if skip_samples > 0 {
                        let skip = skip_samples.min(converted.len()) / ch * ch;
                        let n = (converted.len() - skip).min(producer.free_len()) / ch * ch;
                        let _ = producer.push_slice(&converted[skip..skip + n]);
                        skip_samples -= skip;
                    } else {
                        let n = converted.len().min(producer.free_len()) / ch * ch;
                        let _ = producer.push_slice(&converted[..n]);
                    }
                    if insert_silence > 0 {
                        let n = insert_silence.min(converted.len().max(ch)) / ch * ch;
                        let silence = vec![0.0f32; n];
                        let n = silence.len().min(producer.free_len()) / ch * ch;
                        let _ = producer.push_slice(&silence[..n]);
                        insert_silence = insert_silence.saturating_sub(n);
                    }
                }
                Pop::Gap => { /* next frame position reconstructs the exact missing duration */ }
                Pop::Starved => break,
            }
        }
        metrics
            .buffered_samples
            .store(producer.len() as u64, Ordering::Relaxed);
        // Continuous correction, on a fixed cadence: this loop iterates at an
        // irregular rate driven by packet arrival, so ticking per iteration
        // would make the gains depend on network timing.
        if last_trim_tick.elapsed() >= TRIM_TICK {
            let ppm = trim.update(producer.len() as f32);
            pipeline.set_trim_ppm(ppm);
            trim_ppm_out.store(ppm.round() as i32, Ordering::Relaxed);
            last_trim_tick = Instant::now();
        }
        // Drift observation: once per loop iteration. Corrections are
        // requested in whole device frames so skips never shift channels.
        let drop_count = ((target_samples / 20).max(8)).div_ceil(ch) * ch;
        match drift.observe(producer.len() as f32) {
            super::drift::DriftAction::DropChunk => {
                skip_samples += drop_count;
            }
            super::drift::DriftAction::InsertSilence => {
                insert_silence += drop_count;
            }
            super::drift::DriftAction::None => {}
        }
    }
    let mut goodbye = Vec::new();
    packet::encode_unsubscribe(&initial_info, &psk, &mut goodbye);
    let _ = socket.send(&goodbye);
    running.store(false, Ordering::Relaxed);
}
