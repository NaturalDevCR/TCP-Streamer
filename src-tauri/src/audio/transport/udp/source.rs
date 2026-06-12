//! UDP source I/O: accept one subscriber and stream audio frames to it.

use super::packet::{self, AudioHeader, StreamInfo};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Largest PCM payload per datagram. With the 21-byte header and the 16-byte
/// AEAD tag the whole packet stays under a 1500-byte MTU (no IP
/// fragmentation) and far below the sink's receive buffer. Must be a
/// multiple of 4 so a split never lands inside an s16le stereo frame.
pub const MAX_PAYLOAD_BYTES: usize = 1200;

pub struct UdpSource {
    socket: UdpSocket,
    peer: Option<SocketAddr>,
    seq: u64,
    start: Instant,
    info: StreamInfo,
    last_seen: Instant,
    out: Vec<u8>,
    key: Option<[u8; 32]>,
    nonce_salt: u32,
    psk: String,
}

impl UdpSource {
    /// Binds the UDP source on `port`. Non-blocking so the network loop can
    /// interleave accept + send.
    pub fn bind(port: u16, sample_rate: u32, channels: u16, psk: String) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", port))?;
        socket.set_nonblocking(true)?;
        let salt_a = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        Ok(Self {
            socket,
            peer: None,
            seq: 0,
            start: Instant::now(),
            info: StreamInfo {
                sample_rate,
                channels,
                salt_a,
                flags: 0,
            },
            last_seen: Instant::now(),
            out: Vec::new(),
            key: None,
            nonce_salt: 0,
            psk,
        })
    }

    /// Polls for a SUBSCRIBE/heartbeat. Should be called each loop iteration.
    pub fn poll_subscribe(&mut self) {
        let mut buf = [0u8; 512];
        while let Ok((n, addr)) = self.socket.recv_from(&mut buf) {
            if let Some(salt_b) = packet::decode_subscribe(&buf[..n]) {
                if !self.psk.is_empty() {
                    let key = super::crypto::derive_key(&self.psk, self.info.salt_a, salt_b);
                    self.nonce_salt = super::crypto::nonce_salt(self.info.salt_a, salt_b);
                    self.key = Some(key);
                    self.info.flags = 1; // encrypted
                }
                let mut info_out = Vec::new();
                packet::encode_stream_info(&self.info, &mut info_out);
                let _ = self.socket.send_to(&info_out, addr);
                self.peer = Some(addr);
                self.last_seen = Instant::now();
            }
        }
    }

    /// True if a subscriber is connected and recently seen.
    pub fn has_peer(&self) -> bool {
        self.peer.is_some() && self.last_seen.elapsed() < Duration::from_secs(5)
    }

    /// Sends one block of audio (raw PCM payload) to the subscriber, split
    /// into datagrams of at most [`MAX_PAYLOAD_BYTES`] each (its own seq/ts),
    /// so a large chunk — e.g. a resampled robust-profile chunk — can never
    /// exceed the sink's receive buffer or the path MTU.
    pub fn send_audio(&mut self, payload: &[u8]) {
        if !self.has_peer() {
            self.peer = None;
            return;
        }
        for piece in payload.chunks(MAX_PAYLOAD_BYTES) {
            self.send_datagram(piece);
        }
    }

    fn send_datagram(&mut self, payload: &[u8]) {
        let peer = match self.peer {
            Some(p) => p,
            None => return,
        };
        let ts_us = self.start.elapsed().as_micros() as u64;
        let h = AudioHeader {
            flags: if self.key.is_some() { 1 } else { 0 },
            seq: self.seq,
            ts_us,
        };
        if let Some(key) = &self.key {
            let mut hdr = Vec::new();
            packet::encode_audio(&h, &[], &mut hdr);
            let ct = super::crypto::seal(
                key,
                self.nonce_salt,
                self.seq,
                &hdr[..packet::AUDIO_HEADER_LEN],
                payload,
            );
            packet::encode_audio(&h, &ct, &mut self.out);
        } else {
            packet::encode_audio(&h, payload, &mut self.out);
        }
        let _ = self.socket.send_to(&self.out, peer);
        self.seq = self.seq.wrapping_add(1);
    }

    #[allow(dead_code)]
    pub fn local_addr(&self) -> String {
        self.socket
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_default()
    }
}
