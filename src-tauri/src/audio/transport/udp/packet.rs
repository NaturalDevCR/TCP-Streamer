//! Native UDP wire format: audio + handshake frames (pure encode/decode).

pub const MAGIC: [u8; 2] = *b"TS";
pub const VERSION: u8 = 2;

pub const TYPE_AUDIO: u8 = 0;
pub const TYPE_SUBSCRIBE: u8 = 1;
pub const TYPE_STREAM_INFO: u8 = 2;

pub const TYPE_CONFIRM: u8 = 3;
pub const TYPE_UNSUBSCRIBE: u8 = 4;
pub const TYPE_HEARTBEAT: u8 = 5;
pub const TYPE_CANCEL_SUBSCRIBE: u8 = 6;
pub const AUDIO_HEADER_LEN: usize = 39; // magic2 ver1 type1 flags1 seq8 ts8

/// Audio frame header (payload follows the header in the datagram).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioHeader {
    pub flags: u8,
    pub seq: u64,
    pub ts_us: u64,
    pub session: u64,
    pub frame_pos: u64,
    pub frame_count: u16,
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
    out.extend_from_slice(&h.session.to_be_bytes());
    out.extend_from_slice(&h.frame_pos.to_be_bytes());
    out.extend_from_slice(&h.frame_count.to_be_bytes());
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
    Some((
        AudioHeader {
            flags,
            seq,
            ts_us,
            session: u64::from_be_bytes(buf[21..29].try_into().ok()?),
            frame_pos: u64::from_be_bytes(buf[29..37].try_into().ok()?),
            frame_count: u16::from_be_bytes(buf[37..39].try_into().ok()?),
        },
        &buf[AUDIO_HEADER_LEN..],
    ))
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
    if buf.len() != 12 || buf[0..2] != MAGIC || buf[2] != VERSION || buf[3] != TYPE_SUBSCRIBE {
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
    pub salt_b: u64,
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
    out.extend_from_slice(&info.salt_b.to_be_bytes());
}

pub fn decode_stream_info(buf: &[u8]) -> Option<StreamInfo> {
    if buf.len() != 27 || buf[0..2] != MAGIC || buf[2] != VERSION || buf[3] != TYPE_STREAM_INFO {
        return None;
    }
    Some(StreamInfo {
        sample_rate: u32::from_be_bytes(buf[4..8].try_into().ok()?),
        channels: u16::from_be_bytes(buf[8..10].try_into().ok()?),
        salt_a: u64::from_be_bytes(buf[10..18].try_into().ok()?),
        flags: buf[18],
        salt_b: u64::from_be_bytes(buf[19..27].try_into().ok()?),
    })
}

/// Metadata is validated before opening devices or allocating playback buffers.
pub fn valid_info(info: &StreamInfo) -> bool {
    (8_000..=192_000).contains(&info.sample_rate)
        && (1..=8).contains(&info.channels)
        && info.flags <= 1
}

pub fn encode_authenticated_subscribe(salt_b: u64, psk: &str, out: &mut Vec<u8>) {
    encode_subscribe(salt_b, out);
    authenticate(out, psk, salt_b, 0);
}

pub fn decode_authenticated_subscribe(buf: &[u8], psk: &str) -> Option<u64> {
    if buf.len() != 12 + if psk.is_empty() { 0 } else { 32 } {
        return None;
    }
    let body = verified_body(buf, psk, 0)?;
    decode_subscribe(body)
}

pub fn encode_authenticated_info(info: &StreamInfo, psk: &str, out: &mut Vec<u8>) {
    encode_stream_info(info, out);
    authenticate(out, psk, info.salt_b, 1);
}

pub fn decode_authenticated_info(buf: &[u8], salt_b: u64, psk: &str) -> Option<StreamInfo> {
    if buf.len() != 27 + if psk.is_empty() { 0 } else { 32 } {
        return None;
    }
    let body = verify(buf, psk, salt_b, 1)?;
    let info = decode_stream_info(body)?;
    (valid_info(&info) && info.salt_b == salt_b && (info.flags == 1) == !psk.is_empty())
        .then_some(info)
}

pub fn encode_confirm(info: &StreamInfo, psk: &str, out: &mut Vec<u8>) {
    encode_stream_info(info, out);
    out[3] = TYPE_CONFIRM;
    authenticate(out, psk, info.salt_b, 2);
}

pub fn valid_confirm(buf: &[u8], info: &StreamInfo, psk: &str) -> bool {
    let mut expected = Vec::new();
    encode_confirm(info, psk, &mut expected);
    // HMAC verifies tags in constant time; comparison here only compares public metadata.
    verify(buf, psk, info.salt_b, 2).is_some_and(|body| body == &expected[..27])
}

pub fn encode_unsubscribe(info: &StreamInfo, psk: &str, out: &mut Vec<u8>) {
    encode_stream_info(info, out);
    out[3] = TYPE_UNSUBSCRIBE;
    authenticate(out, psk, info.salt_b, 3);
}

pub fn valid_unsubscribe(buf: &[u8], info: &StreamInfo, psk: &str) -> bool {
    let mut expected = Vec::new();
    encode_unsubscribe(info, psk, &mut expected);
    verify(buf, psk, info.salt_b, 3).is_some_and(|body| body == &expected[..27])
}

pub fn encode_heartbeat(info: &StreamInfo, counter: u64, psk: &str, out: &mut Vec<u8>) {
    encode_stream_info(info, out);
    out[3] = TYPE_HEARTBEAT;
    out.extend_from_slice(&counter.to_be_bytes());
    authenticate(out, psk, info.salt_b, 4);
}

pub fn decode_heartbeat(buf: &[u8], info: &StreamInfo, psk: &str) -> Option<u64> {
    let body = verify(buf, psk, info.salt_b, 4)?;
    if body.len() != 35 {
        return None;
    }
    let counter = u64::from_be_bytes(body[27..35].try_into().ok()?);
    let mut expected = Vec::new();
    encode_heartbeat(info, counter, psk, &mut expected);
    (body == &expected[..35]).then_some(counter)
}

pub fn encode_cancel_subscribe(salt_b: u64, psk: &str, out: &mut Vec<u8>) {
    encode_subscribe(salt_b, out);
    out[3] = TYPE_CANCEL_SUBSCRIBE;
    authenticate(out, psk, salt_b, 5);
}

pub fn valid_cancel_subscribe(buf: &[u8], salt_b: u64, psk: &str) -> bool {
    let mut expected = Vec::new();
    encode_cancel_subscribe(salt_b, psk, &mut expected);
    verify(buf, psk, salt_b, 5).is_some_and(|body| body == &expected[..12])
}

fn handshake_mac(psk: &str, salt_b: u64, purpose: u64, body: &[u8]) -> hmac::Hmac<sha2::Sha256> {
    use hmac::Mac;
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(psk.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(b"tcp-streamer/udp/v2/handshake");
    mac.update(&salt_b.to_be_bytes());
    mac.update(&purpose.to_be_bytes());
    mac.update(body);
    mac
}

fn authenticate(out: &mut Vec<u8>, psk: &str, salt_b: u64, purpose: u64) {
    use hmac::Mac;
    if !psk.is_empty() {
        let tag = handshake_mac(psk, salt_b, purpose, out)
            .finalize()
            .into_bytes();
        out.extend_from_slice(&tag);
    }
}

fn verified_body<'a>(buf: &'a [u8], psk: &str, purpose: u64) -> Option<&'a [u8]> {
    if buf.len() < 12 {
        return None;
    }
    let salt_b = u64::from_be_bytes(buf[4..12].try_into().ok()?);
    verify(buf, psk, salt_b, purpose)
}

fn verify<'a>(buf: &'a [u8], psk: &str, salt_b: u64, purpose: u64) -> Option<&'a [u8]> {
    use hmac::Mac;
    if psk.is_empty() {
        return Some(buf);
    }
    let body_len = buf.len().checked_sub(32)?;
    let (body, tag) = buf.split_at(body_len);
    handshake_mac(psk, salt_b, purpose, body)
        .verify_slice(tag)
        .ok()?;
    Some(body)
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
            session: 1,
            frame_pos: 0,
            frame_count: 1,
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
            salt_b: 7,
            flags: 0,
        };
        let mut out = Vec::new();
        encode_stream_info(&info, &mut out);
        assert_eq!(decode_stream_info(&out), Some(info));
    }
}
