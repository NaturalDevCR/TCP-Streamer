//! Native UDP wire format: audio + handshake frames (pure encode/decode).

pub const MAGIC: [u8; 2] = *b"TS";
pub const VERSION: u8 = 1;

pub const TYPE_AUDIO: u8 = 0;
pub const TYPE_SUBSCRIBE: u8 = 1;
pub const TYPE_STREAM_INFO: u8 = 2;

pub const AUDIO_HEADER_LEN: usize = 21; // magic2 ver1 type1 flags1 seq8 ts8

/// Audio frame header (payload follows the header in the datagram).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioHeader {
    pub flags: u8,
    pub seq: u64,
    pub ts_us: u64,
}

/// Writes an AUDIO datagram (header + payload) into `out`.
pub fn encode_audio(h: &AudioHeader, payload: &[u8], out: &mut Vec<u8>) {
    out.clear();
    out.reserve(AUDIO_HEADER_LEN + payload.len());
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    out.push(TYPE_AUDIO);
    out.push(h.flags);
    out.extend_from_slice(&h.seq.to_be_bytes());
    out.extend_from_slice(&h.ts_us.to_be_bytes());
    out.extend_from_slice(payload);
}

/// Parses an AUDIO datagram, returning the header and the payload slice.
pub fn decode_audio(buf: &[u8]) -> Option<(AudioHeader, &[u8])> {
    if buf.len() < AUDIO_HEADER_LEN
        || buf[0..2] != MAGIC
        || buf[2] != VERSION
        || buf[3] != TYPE_AUDIO
    {
        return None;
    }
    let flags = buf[4];
    let seq = u64::from_be_bytes(buf[5..13].try_into().ok()?);
    let ts_us = u64::from_be_bytes(buf[13..21].try_into().ok()?);
    Some((AudioHeader { flags, seq, ts_us }, &buf[AUDIO_HEADER_LEN..]))
}

/// SUBSCRIBE: a sink asks a source to start streaming. `salt_b` is reserved for
/// 2B-ii (encryption); unused in plaintext mode.
pub fn encode_subscribe(salt_b: u64, out: &mut Vec<u8>) {
    out.clear();
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    out.push(TYPE_SUBSCRIBE);
    out.extend_from_slice(&salt_b.to_be_bytes());
}

pub fn decode_subscribe(buf: &[u8]) -> Option<u64> {
    if buf.len() < 12 || buf[0..2] != MAGIC || buf[2] != VERSION || buf[3] != TYPE_SUBSCRIBE {
        return None;
    }
    Some(u64::from_be_bytes(buf[4..12].try_into().ok()?))
}

/// STREAM_INFO: a source announces the stream format to a subscribing sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub salt_a: u64,
    pub flags: u8,
}

pub fn encode_stream_info(info: &StreamInfo, out: &mut Vec<u8>) {
    out.clear();
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    out.push(TYPE_STREAM_INFO);
    out.extend_from_slice(&info.sample_rate.to_be_bytes());
    out.extend_from_slice(&info.channels.to_be_bytes());
    out.extend_from_slice(&info.salt_a.to_be_bytes());
    out.push(info.flags);
}

pub fn decode_stream_info(buf: &[u8]) -> Option<StreamInfo> {
    if buf.len() < 19 || buf[0..2] != MAGIC || buf[2] != VERSION || buf[3] != TYPE_STREAM_INFO {
        return None;
    }
    Some(StreamInfo {
        sample_rate: u32::from_be_bytes(buf[4..8].try_into().ok()?),
        channels: u16::from_be_bytes(buf[8..10].try_into().ok()?),
        salt_a: u64::from_be_bytes(buf[10..18].try_into().ok()?),
        flags: buf[18],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_roundtrip() {
        let h = AudioHeader {
            flags: 0,
            seq: 42,
            ts_us: 123456,
        };
        let mut out = Vec::new();
        encode_audio(&h, &[1, 2, 3, 4], &mut out);
        let (got, payload) = decode_audio(&out).unwrap();
        assert_eq!(got, h);
        assert_eq!(payload, &[1, 2, 3, 4]);
    }

    #[test]
    fn rejects_bad_magic_version_type_or_short() {
        assert!(decode_audio(&[0, 0, 0]).is_none());
        let mut out = Vec::new();
        encode_subscribe(7, &mut out);
        assert!(decode_audio(&out).is_none()); // wrong type
        assert_eq!(decode_subscribe(&out), Some(7));
    }

    #[test]
    fn stream_info_roundtrip() {
        let info = StreamInfo {
            sample_rate: 48000,
            channels: 2,
            salt_a: 99,
            flags: 0,
        };
        let mut out = Vec::new();
        encode_stream_info(&info, &mut out);
        assert_eq!(decode_stream_info(&out), Some(info));
    }
}
