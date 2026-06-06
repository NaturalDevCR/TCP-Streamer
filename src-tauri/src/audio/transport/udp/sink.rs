//! UDP sink I/O: subscribe to a source, receive audio, de-jitter, decode, and
//! feed the playback ring buffer.

use super::{jitter::{JitterBuffer, Pop}, packet};
use crate::audio::engine::decoder::decode_pcm_i16_le_to_f32;
use ringbuf::HeapProducer;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Result of the subscribe handshake.
pub struct Subscribed {
    pub socket: UdpSocket,
    pub info: packet::StreamInfo,
}

/// Sends SUBSCRIBE to `source_addr` and waits for STREAM_INFO. Returns the bound
/// socket (connected to the source) and the negotiated format.
pub fn subscribe(source_addr: &str, salt_b: u64, timeout: Duration) -> std::io::Result<Subscribed> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(source_addr)?;
    socket.set_read_timeout(Some(timeout))?;

    let mut sub = Vec::new();
    packet::encode_subscribe(salt_b, &mut sub);

    let mut buf = [0u8; 2048];
    let deadline = Instant::now() + timeout * 3;
    loop {
        socket.send(&sub)?;
        match socket.recv(&mut buf) {
            Ok(n) => {
                if let Some(info) = packet::decode_stream_info(&buf[..n]) {
                    socket.set_read_timeout(Some(Duration::from_millis(500)))?;
                    return Ok(Subscribed { socket, info });
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(e),
        }
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "no STREAM_INFO from source"));
        }
    }
}

/// Receives audio packets until `running` is cleared, decoding into `producer`.
/// Resends SUBSCRIBE every ~1s as a heartbeat.
pub fn receive_loop(
    socket: &UdpSocket,
    salt_b: u64,
    lost_after: usize,
    mut producer: HeapProducer<f32>,
    running: Arc<AtomicBool>,
) {
    let mut jb = JitterBuffer::new(lost_after);
    let mut buf = [0u8; 4096];
    let mut decoded: Vec<f32> = Vec::new();
    let mut sub = Vec::new();
    packet::encode_subscribe(salt_b, &mut sub);
    let mut last_hb = Instant::now();

    while running.load(Ordering::Relaxed) {
        if last_hb.elapsed() >= Duration::from_secs(1) {
            let _ = socket.send(&sub);
            last_hb = Instant::now();
        }
        match socket.recv(&mut buf) {
            Ok(n) => {
                if let Some((h, payload)) = packet::decode_audio(&buf[..n]) {
                    jb.push(h.seq, payload.to_vec());
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => break,
        }
        // Drain the jitter buffer into the playback ring.
        loop {
            match jb.pop() {
                Pop::Frame(pcm) => {
                    decode_pcm_i16_le_to_f32(&pcm, &mut decoded);
                    let _ = producer.push_slice(&decoded);
                }
                Pop::Gap => { /* silence handled by the output callback's underrun fill */ }
                Pop::Starved => break,
            }
        }
    }
}
