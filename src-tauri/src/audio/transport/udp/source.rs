//! UDP source I/O: accept one subscriber and stream audio frames to it.

use super::packet::{self, AudioHeader, StreamInfo};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// Largest PCM payload per datagram. With the 39-byte header and the 16-byte
/// AEAD tag the whole packet stays under a 1500-byte MTU (no IP
/// fragmentation) and far below the sink's receive buffer. Must be a
/// multiple of 4 so a split never lands inside an s16le stereo frame.
pub const MAX_PAYLOAD_BYTES: usize = 1200;

pub struct UdpSource {
    socket: UdpSocket,
    peer: Option<SocketAddr>,
    seq: u64,
    frame_pos: u64,
    heartbeat_counter: Option<u64>,
    pending: Option<(SocketAddr, StreamInfo, Instant)>,
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
        if !(8_000..=192_000).contains(&sample_rate) || !(1..=8).contains(&channels) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "unsupported UDP format",
            ));
        }
        // On macOS an IPv6 bind may succeed beside a competing IPv4 listener.
        // Refuse that ambiguity before opening the dual-stack socket.
        if port != 0 {
            drop(UdpSocket::bind(("0.0.0.0", port))?);
        }
        // Dual-stack where supported; fall back to IPv4 on hosts without IPv6.
        let socket = socket2::Socket::new(
            socket2::Domain::IPV6,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        )
        .and_then(|socket| {
            socket.set_only_v6(false)?;
            socket.bind(&SocketAddr::from(([0u16; 8], port)).into())?;
            Ok(UdpSocket::from(socket))
        })
        .or_else(|_| UdpSocket::bind(("0.0.0.0", port)))?;
        socket.set_nonblocking(true)?;
        let salt_a = super::crypto::random_salt()?;
        Ok(Self {
            socket,
            peer: None,
            seq: 0,
            frame_pos: 0,
            heartbeat_counter: None,
            pending: None,
            start: Instant::now(),
            info: StreamInfo {
                sample_rate,
                channels,
                salt_a,
                salt_b: 0,
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
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, _, t)| t.elapsed() >= Duration::from_secs(5))
        {
            self.pending = None;
        }
        let mut buf = [0u8; 512];
        // Bound control-plane work so subscription floods cannot starve audio.
        for _ in 0..64 {
            let Ok((n, addr)) = self.socket.recv_from(&mut buf) else {
                break;
            };
            if let Some((pending_addr, info, _)) = self.pending {
                if pending_addr == addr
                    && (packet::valid_cancel_subscribe(&buf[..n], info.salt_b, &self.psk)
                        || packet::valid_unsubscribe(&buf[..n], &info, &self.psk))
                {
                    self.pending = None;
                    continue;
                }
            }
            if self.peer == Some(addr) {
                if let Some(counter) = packet::decode_heartbeat(&buf[..n], &self.info, &self.psk) {
                    if self
                        .heartbeat_counter
                        .is_none_or(|previous| counter > previous)
                    {
                        self.heartbeat_counter = Some(counter);
                        self.last_seen = Instant::now();
                    }
                    continue;
                }
            }
            if self.peer == Some(addr)
                && packet::valid_unsubscribe(&buf[..n], &self.info, &self.psk)
            {
                self.peer = None;
                self.key = None;
                self.pending = None;
                continue;
            }
            if self.has_peer() && self.peer != Some(addr) {
                continue;
            }
            if let Some(salt_b) = packet::decode_authenticated_subscribe(&buf[..n], &self.psk) {
                let info = if self.has_peer() && self.info.salt_b == salt_b {
                    self.info
                } else if let Some((pending_addr, info, started)) = self.pending {
                    if pending_addr == addr
                        && info.salt_b == salt_b
                        && started.elapsed() < Duration::from_secs(5)
                    {
                        info
                    } else {
                        continue;
                    }
                } else {
                    let Ok(salt_a) = super::crypto::random_salt() else {
                        continue;
                    };
                    let info = StreamInfo {
                        salt_a,
                        salt_b,
                        flags: u8::from(!self.psk.is_empty()),
                        ..self.info
                    };
                    self.pending = Some((addr, info, Instant::now()));
                    info
                };
                let mut info_out = Vec::new();
                packet::encode_authenticated_info(&info, &self.psk, &mut info_out);
                let _ = self.socket.send_to(&info_out, addr);
            } else if let Some((pending_addr, info, started)) = self.pending {
                if pending_addr == addr
                    && started.elapsed() < Duration::from_secs(5)
                    && packet::valid_confirm(&buf[..n], &info, &self.psk)
                {
                    self.info = info;
                    self.key = (!self.psk.is_empty())
                        .then(|| super::crypto::derive_key(&self.psk, info.salt_a, info.salt_b));
                    self.nonce_salt = super::crypto::nonce_salt(info.salt_a, info.salt_b);
                    self.seq = 0;
                    self.frame_pos = 0;
                    self.heartbeat_counter = None;
                    self.peer = Some(addr);
                    self.last_seen = Instant::now();
                    self.pending = None;
                }
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
    pub fn send_audio(&mut self, payload: &[u8]) -> std::io::Result<usize> {
        if !self.has_peer() {
            self.peer = None;
            return Ok(0);
        }
        let frame_bytes = self.info.channels as usize * 2;
        if !payload.len().is_multiple_of(frame_bytes) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "PCM must contain whole frames",
            ));
        }
        let piece_bytes = MAX_PAYLOAD_BYTES / frame_bytes * frame_bytes;
        let mut sent = 0;
        for piece in payload.chunks(piece_bytes) {
            self.send_datagram(piece)?;
            sent += piece.len();
        }
        Ok(sent)
    }

    fn send_datagram(&mut self, payload: &[u8]) -> std::io::Result<()> {
        let peer = match self.peer {
            Some(p) => p,
            None => return Ok(()),
        };
        let ts_us = self.start.elapsed().as_micros() as u64;
        let h = AudioHeader {
            flags: if self.key.is_some() { 1 } else { 0 },
            seq: self.seq,
            ts_us,
            session: self.info.salt_a,
            frame_pos: self.frame_pos,
            frame_count: (payload.len() / (self.info.channels as usize * 2)) as u16,
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
        // Reserve nonce/timeline before sending: an I/O failure must never
        // allow the next PCM block to reuse the same AEAD nonce.
        self.seq = self
            .seq
            .checked_add(1)
            .ok_or_else(|| std::io::Error::other("UDP sequence exhausted"))?;
        self.frame_pos += h.frame_count as u64;
        self.socket.send_to(&self.out, peer)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn local_addr(&self) -> String {
        self.socket
            .local_addr()
            .map(|a| {
                if a.is_ipv6() {
                    format!("[::1]:{}", a.port())
                } else {
                    format!("127.0.0.1:{}", a.port())
                }
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replayed_heartbeats_cannot_extend_a_dead_peer_lease() {
        let mut source = UdpSource::bind(0, 48000, 2, "secret".into()).unwrap();
        let address: SocketAddr = source.local_addr().parse().unwrap();
        let client = UdpSocket::bind(if address.is_ipv6() {
            "[::1]:0"
        } else {
            "127.0.0.1:0"
        })
        .unwrap();
        client.connect(address).unwrap();
        source.peer = Some(client.local_addr().unwrap());
        source.info.salt_b = 2;
        source.info.flags = 1;
        source.heartbeat_counter = Some(5);
        source.last_seen = Instant::now() - Duration::from_secs(6);
        let mut packet = Vec::new();
        packet::encode_heartbeat(&source.info, 5, "secret", &mut packet);
        client.send(&packet).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        source.poll_subscribe();
        assert!(!source.has_peer());
        packet::encode_heartbeat(&source.info, 6, "secret", &mut packet);
        client.send(&packet).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        source.poll_subscribe();
        assert!(source.has_peer());
    }
    #[test]
    fn cancelling_pending_handshake_requires_authentication_and_releases_slot() {
        let mut source = UdpSource::bind(0, 48000, 2, "secret".into()).unwrap();
        let address: SocketAddr = source.local_addr().parse().unwrap();
        let client = UdpSocket::bind(if address.is_ipv6() {
            "[::1]:0"
        } else {
            "127.0.0.1:0"
        })
        .unwrap();
        client.connect(address).unwrap();
        let info = packet::StreamInfo {
            salt_b: 2,
            flags: 1,
            ..source.info
        };
        source.pending = Some((client.local_addr().unwrap(), info, Instant::now()));
        let mut packet = Vec::new();
        packet::encode_cancel_subscribe(2, "wrong", &mut packet);
        client.send(&packet).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        source.poll_subscribe();
        assert!(source.pending.is_some());
        packet::encode_cancel_subscribe(2, "secret", &mut packet);
        client.send(&packet).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        source.poll_subscribe();
        assert!(source.pending.is_none());
    }
}
