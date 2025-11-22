// Protocol definitions for TCP Streamer
// Packet format with timestamping and sequence numbers

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Magic bytes to identify TCP Streamer packets
pub const MAGIC: &[u8; 4] = b"TCPS";

/// Packet header structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketHeader {
    /// Magic bytes: "TCPS"
    pub magic: [u8; 4],
    /// Timestamp in microseconds since UNIX epoch
    pub timestamp_us: u64,
    /// Sequence number (wraps at u32::MAX)
    pub sequence: u32,
    /// Payload size in bytes
    pub payload_size: u32,
    /// Codec used (0=None, 1=Flac, 2=Opus)
    pub codec: u8,
}

impl PacketHeader {
    pub const SIZE: usize = 4 + 8 + 4 + 4 + 1; // 21 bytes

    pub fn new(sequence: u32, payload_size: u32, codec: u8) -> Self {
        let timestamp_us = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        Self {
            magic: *MAGIC,
            timestamp_us,
            sequence,
            payload_size,
            codec,
        }
    }

    /// Serialize header to bytes (little-endian)
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0..4].copy_from_slice(&self.magic);
        bytes[4..12].copy_from_slice(&self.timestamp_us.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.payload_size.to_le_bytes());
        bytes[20] = self.codec;
        bytes
    }

    /// Deserialize header from bytes
    #[allow(dead_code)]
    pub fn from_bytes(bytes: &[u8; Self::SIZE]) -> Option<Self> {
        if &bytes[0..4] != MAGIC {
            return None;
        }

        Some(Self {
            magic: [bytes[0], bytes[1], bytes[2], bytes[3]],
            timestamp_us: u64::from_le_bytes(bytes[4..12].try_into().ok()?),
            sequence: u32::from_le_bytes(bytes[12..16].try_into().ok()?),
            payload_size: u32::from_le_bytes(bytes[16..20].try_into().ok()?),
            codec: bytes[20],
        })
    }
}

/// Complete packet with header and payload
pub struct Packet {
    pub header: PacketHeader,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn new(sequence: u32, payload: Vec<u8>, codec: u8) -> Self {
        let payload_size = payload.len() as u32;
        Self {
            header: PacketHeader::new(sequence, payload_size, codec),
            payload,
        }
    }

    /// Serialize entire packet to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let header_bytes = self.header.to_bytes();
        let mut packet = Vec::with_capacity(PacketHeader::SIZE + self.payload.len());
        packet.extend_from_slice(&header_bytes);
        packet.extend_from_slice(&self.payload);
        packet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_serialization() {
        let header = PacketHeader::new(42, 1024, 0);
        let bytes = header.to_bytes();
        let parsed = PacketHeader::from_bytes(&bytes).expect("Failed to parse header");

        assert_eq!(&parsed.magic, MAGIC);
        assert_eq!(parsed.sequence, 42);
        assert_eq!(parsed.payload_size, 1024);
        assert_eq!(parsed.codec, 0);
    }

    #[test]
    fn test_packet_creation() {
        let payload = vec![1, 2, 3, 4];
        let packet = Packet::new(100, payload.clone(), 1);

        assert_eq!(packet.header.sequence, 100);
        assert_eq!(packet.header.payload_size, 4);
        assert_eq!(packet.header.codec, 1);
        assert_eq!(packet.payload, payload);
    }
}
