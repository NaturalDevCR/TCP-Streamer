//! End-to-end UDP test over localhost: a UdpSource and a subscribing socket
//! exchange the handshake and one audio frame.

use super::{packet, source::UdpSource};
use std::net::UdpSocket;
use std::time::Duration;

#[test]
fn subscribe_then_receive_one_audio_frame() {
    let mut src = UdpSource::bind(0, 48000, 2, String::new()).expect("bind");
    let src_addr = src.local_addr();

    let sink = UdpSocket::bind("127.0.0.1:0").unwrap();
    sink.connect(src_addr).unwrap();
    sink.set_read_timeout(Some(Duration::from_secs(1))).unwrap();

    // SUBSCRIBE
    let mut sub = Vec::new();
    packet::encode_subscribe(123, &mut sub);
    sink.send(&sub).unwrap();

    // Source handles it
    std::thread::sleep(Duration::from_millis(50));
    src.poll_subscribe();
    assert!(src.has_peer(), "source should have a peer after subscribe");

    // STREAM_INFO should have arrived
    let mut buf = [0u8; 2048];
    let n = sink.recv(&mut buf).unwrap();
    let info = packet::decode_stream_info(&buf[..n]).expect("stream info");
    assert_eq!(info.sample_rate, 48000);
    assert_eq!(info.channels, 2);

    // Source sends one audio frame
    src.send_audio(&[1, 2, 3, 4]);
    let n = sink.recv(&mut buf).unwrap();
    let (h, payload) = packet::decode_audio(&buf[..n]).expect("audio");
    assert_eq!(h.seq, 0);
    assert_eq!(payload, &[1, 2, 3, 4]);
}

#[test]
fn large_chunks_are_split_into_bounded_sequential_datagrams() {
    use super::source::MAX_PAYLOAD_BYTES;

    let mut src = UdpSource::bind(0, 48000, 2, String::new()).expect("bind");
    let src_addr = src.local_addr();

    let sink = UdpSocket::bind("127.0.0.1:0").unwrap();
    sink.connect(src_addr).unwrap();
    sink.set_read_timeout(Some(Duration::from_secs(1))).unwrap();

    let mut sub = Vec::new();
    packet::encode_subscribe(7, &mut sub);
    sink.send(&sub).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    src.poll_subscribe();
    let mut buf = [0u8; 65535];
    let _ = sink.recv(&mut buf).unwrap(); // STREAM_INFO

    // A robust-profile resampled chunk (~5000 bytes) must arrive complete,
    // in order, with every datagram within the bound (this used to be one
    // 5000-byte datagram that a 4096-byte receive buffer truncated).
    let big: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    src.send_audio(&big);

    let mut reassembled = Vec::new();
    let mut expected_seq = 0u64;
    while reassembled.len() < big.len() {
        let n = sink.recv(&mut buf).expect("datagram");
        let (h, payload) = packet::decode_audio(&buf[..n]).expect("audio");
        assert_eq!(h.seq, expected_seq, "datagrams must be sequential");
        assert!(
            payload.len() <= MAX_PAYLOAD_BYTES,
            "datagram payload {} exceeds bound {}",
            payload.len(),
            MAX_PAYLOAD_BYTES
        );
        reassembled.extend_from_slice(payload);
        expected_seq += 1;
    }
    assert_eq!(reassembled, big, "payload must reassemble exactly");
    assert_eq!(expected_seq, 5, "5000 bytes at 1200/packet = 5 datagrams");
    assert_eq!(MAX_PAYLOAD_BYTES % 4, 0, "split must stay frame-aligned");
}

#[test]
fn encrypted_frame_roundtrips_and_wrong_psk_fails() {
    use super::crypto::{derive_key, nonce_salt, open, seal};
    let (sa, sb) = (0x1111_2222_3333_4444u64, 0xAAAA_BBBB_CCCC_DDDDu64);
    let k = derive_key("hunter2", sa, sb);
    let ns = nonce_salt(sa, sb);
    let aad = b"audio-header";
    let ct = seal(&k, ns, 7, aad, b"pcm-frame");
    assert_eq!(
        open(&k, ns, 7, aad, &ct).as_deref(),
        Some(&b"pcm-frame"[..])
    );
    let kbad = derive_key("wrong", sa, sb);
    assert!(open(&kbad, ns, 7, aad, &ct).is_none());
}
