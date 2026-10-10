//! End-to-end UDP test over localhost: a UdpSource and a subscribing socket
//! exchange the handshake and one audio frame.

use super::{packet, source::UdpSource};
use std::net::UdpSocket;
use std::time::Duration;

#[test]
fn subscribe_then_receive_one_audio_frame() {
    let mut src = UdpSource::bind(0, 48000, 2, String::new()).expect("bind");
    let sink = client(&src);
    sink.set_read_timeout(Some(Duration::from_secs(1))).unwrap();

    // SUBSCRIBE
    let mut sub = Vec::new();
    packet::encode_subscribe(123, &mut sub);
    sink.send(&sub).unwrap();

    // Source handles it
    std::thread::sleep(Duration::from_millis(50));
    src.poll_subscribe();

    // STREAM_INFO should have arrived
    let mut buf = [0u8; 2048];
    let n = sink.recv(&mut buf).unwrap();
    let info = packet::decode_stream_info(&buf[..n]).expect("stream info");
    assert_eq!(info.sample_rate, 48000);
    assert_eq!(info.channels, 2);
    packet::encode_confirm(&info, "", &mut sub);
    sink.send(&sub).unwrap();
    std::thread::sleep(Duration::from_millis(10));
    src.poll_subscribe();
    assert!(src.has_peer());

    // Source sends one audio frame
    src.send_audio(&[1, 2, 3, 4]).unwrap();
    let n = sink.recv(&mut buf).unwrap();
    let (h, payload) = packet::decode_audio(&buf[..n]).expect("audio");
    assert_eq!(h.seq, 0);
    assert_eq!(payload, &[1, 2, 3, 4]);
}

#[test]
fn large_chunks_are_split_into_bounded_sequential_datagrams() {
    use super::source::MAX_PAYLOAD_BYTES;

    let mut src = UdpSource::bind(0, 48000, 2, String::new()).expect("bind");
    let sink = client(&src);
    sink.set_read_timeout(Some(Duration::from_secs(1))).unwrap();

    let mut sub = Vec::new();
    packet::encode_subscribe(7, &mut sub);
    sink.send(&sub).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    src.poll_subscribe();
    let mut buf = [0u8; 65535];
    let n = sink.recv(&mut buf).unwrap();
    let info = packet::decode_stream_info(&buf[..n]).unwrap();
    packet::encode_confirm(&info, "", &mut sub);
    sink.send(&sub).unwrap();
    std::thread::sleep(Duration::from_millis(10));
    src.poll_subscribe();

    // A robust-profile resampled chunk (~5000 bytes) must arrive complete,
    // in order, with every datagram within the bound (this used to be one
    // 5000-byte datagram that a 4096-byte receive buffer truncated).
    let big: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    src.send_audio(&big).unwrap();

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

fn request(
    src: &mut UdpSource,
    sink: &UdpSocket,
    salt: u64,
    psk: &str,
) -> Option<packet::StreamInfo> {
    let mut request = Vec::new();
    packet::encode_authenticated_subscribe(salt, psk, &mut request);
    sink.send(&request).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    src.poll_subscribe();
    let mut buf = [0; 256];
    let n = sink.recv(&mut buf).ok()?;
    packet::decode_authenticated_info(&buf[..n], salt, psk)
}

fn client(src: &UdpSource) -> UdpSocket {
    let addr: std::net::SocketAddr = src.local_addr().parse().unwrap();
    let socket = UdpSocket::bind(if addr.is_ipv6() {
        "[::1]:0"
    } else {
        "127.0.0.1:0"
    })
    .unwrap();
    socket.connect(addr).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    socket
}

#[test]
fn authenticated_handshake_rejects_wrong_psk_and_requires_fresh_confirmation() {
    let mut src = UdpSource::bind(0, 48000, 2, "secret".into()).unwrap();
    let socket = client(&src);
    assert!(request(&mut src, &socket, 1, "wrong").is_none());
    assert!(request(&mut src, &socket, 1, "").is_none());
    assert!(!src.has_peer());
    let info = request(&mut src, &socket, 2, "secret").unwrap();
    assert!(!src.has_peer(), "request alone cannot acquire a peer");
    let mut confirmation = Vec::new();
    packet::encode_confirm(&info, "wrong", &mut confirmation);
    socket.send(&confirmation).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    src.poll_subscribe();
    assert!(!src.has_peer());
    packet::encode_confirm(&info, "secret", &mut confirmation);
    socket.send(&confirmation).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    src.poll_subscribe();
    assert!(src.has_peer());
    let second = client(&src);
    assert!(request(&mut src, &second, 3, "secret").is_none());
    src.send_audio(&[1, 2, 3, 4]).unwrap();
    let mut buf = [0; 256];
    let n = socket.recv(&mut buf).unwrap();
    let (header, cipher) = packet::decode_audio(&buf[..n]).unwrap();
    let key = super::crypto::derive_key("secret", info.salt_a, 2);
    let salt = super::crypto::nonce_salt(info.salt_a, 2);
    assert_eq!(
        super::crypto::open(
            &key,
            salt,
            header.seq,
            &buf[..packet::AUDIO_HEADER_LEN],
            cipher
        )
        .unwrap(),
        [1, 2, 3, 4]
    );
    packet::encode_unsubscribe(&info, "wrong", &mut confirmation);
    socket.send(&confirmation).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    src.poll_subscribe();
    assert!(src.has_peer());
    packet::encode_unsubscribe(&info, "secret", &mut confirmation);
    socket.send(&confirmation).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    src.poll_subscribe();
    assert!(!src.has_peer());
}

#[test]
fn forged_high_sequence_and_plaintext_do_not_poison_authenticated_receiver() {
    use super::crypto::*;
    let key = derive_key("secret", 1, 2);
    let mut replay = ReplayWindow::new();
    let mut header = packet::AudioHeader {
        flags: 1,
        seq: 1_000_000,
        ts_us: 0,
        session: 1,
        frame_pos: 0,
        frame_count: 1,
    };
    let mut aad = Vec::new();
    packet::encode_audio(&header, &[], &mut aad);
    assert!(authenticated_audio(Some(&key), 0, &header, &aad, &[0; 20], &mut replay).is_none());
    header.flags = 0;
    packet::encode_audio(&header, &[], &mut aad);
    assert!(authenticated_audio(Some(&key), 0, &header, &aad, &[0; 4], &mut replay).is_none());
    header.flags = 1;
    header.seq = 0;
    packet::encode_audio(&header, &[], &mut aad);
    let cipher = seal(&key, 0, 0, &aad, &[1, 2, 3, 4]);
    assert_eq!(
        authenticated_audio(Some(&key), 0, &header, &aad, &cipher, &mut replay).unwrap(),
        [1, 2, 3, 4]
    );
    assert!(authenticated_audio(Some(&key), 0, &header, &aad, &cipher, &mut replay).is_none());
}

#[test]
fn restarted_source_negotiates_new_salt_and_full_format() {
    let mut src = UdpSource::bind(0, 48000, 2, "secret".into()).unwrap();
    let socket = client(&src);
    let old = request(&mut src, &socket, 9, "secret").unwrap();
    let addr: std::net::SocketAddr = src.local_addr().parse().unwrap();
    drop(src);
    let mut restarted = UdpSource::bind(addr.port(), 44100, 1, "secret".into()).unwrap();
    let new = request(&mut restarted, &socket, 9, "secret").unwrap();
    assert_ne!(old.salt_a, new.salt_a);
    assert_eq!((new.sample_rate, new.channels), (44100, 1));
    assert_ne!(
        super::crypto::derive_key("secret", old.salt_a, 9),
        super::crypto::derive_key("secret", new.salt_a, 9)
    );
    let mut confirm = Vec::new();
    packet::encode_confirm(&old, "secret", &mut confirm);
    socket.send(&confirm).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    restarted.poll_subscribe();
    assert!(!restarted.has_peer());
    packet::encode_confirm(&new, "secret", &mut confirm);
    socket.send(&confirm).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    restarted.poll_subscribe();
    assert!(restarted.has_peer());
}

#[test]
fn jitter_late_join_and_discontinuity_have_bounded_work_and_memory() {
    use super::jitter::{JitterBuffer, Pop};
    let mut jitter = JitterBuffer::new(3);
    jitter.push(1_000_000, vec![1]);
    assert_eq!(jitter.pop(), Pop::Frame(vec![1]));
    jitter.push(u64::MAX - 1, vec![2]);
    assert_eq!(jitter.pop(), Pop::Frame(vec![2]));
    for seq in 0..1000 {
        jitter.push(seq, vec![0]);
    }
    assert!(jitter.buffered() <= 256);
}

#[test]
fn metadata_rejects_invalid_format_unknown_flags_and_trailing_bytes() {
    let valid = packet::StreamInfo {
        sample_rate: 48000,
        channels: 2,
        salt_a: 1,
        salt_b: 2,
        flags: 1,
    };
    let mut buf = Vec::new();
    for bad in [
        packet::StreamInfo {
            sample_rate: 0,
            ..valid
        },
        packet::StreamInfo {
            channels: 65535,
            ..valid
        },
        packet::StreamInfo { flags: 2, ..valid },
    ] {
        packet::encode_authenticated_info(&bad, "secret", &mut buf);
        assert!(packet::decode_authenticated_info(&buf, 2, "secret").is_none());
    }
    packet::encode_authenticated_info(&valid, "secret", &mut buf);
    assert!(packet::decode_authenticated_info(&buf, 2, "wrong").is_none());
    assert!(packet::decode_authenticated_info(&buf, 2, "").is_none());
    buf.push(0);
    assert!(packet::decode_authenticated_info(&buf, 2, "secret").is_none());
}

#[test]
fn receive_loop_rekeys_after_source_restart_and_preserves_variable_packet_gaps() {
    use super::crypto::{derive_key, nonce_salt, seal};
    use std::sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        Arc,
    };
    let source = UdpSocket::bind("127.0.0.1:0").unwrap();
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver.connect(source.local_addr().unwrap()).unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_millis(10)))
        .unwrap();
    let destination = receiver.local_addr().unwrap();
    let info = packet::StreamInfo {
        sample_rate: 48000,
        channels: 2,
        salt_a: 11,
        salt_b: 22,
        flags: 1,
    };
    let rb = ringbuf::HeapRb::<f32>::new(20000);
    let (producer, mut consumer) = rb.split();
    let running = Arc::new(AtomicBool::new(true));
    let worker_running = running.clone();
    let metrics = Arc::new(crate::audio::stats::ReceiveMetrics::default());
    let worker_metrics = metrics.clone();
    let worker = std::thread::spawn(move || {
        super::sink::receive_loop(
            &receiver,
            22,
            1,
            "secret".into(),
            info,
            0,
            48000,
            2,
            crate::audio::engine::convert::SinkPipeline::new(48000, 2, 48000, 2),
            producer,
            worker_running,
            Arc::new(AtomicI32::new(0)),
            worker_metrics,
        )
    });
    let send = |info: packet::StreamInfo, seq, pos, frames: usize, sample: i16| {
        let header = packet::AudioHeader {
            flags: 1,
            seq,
            ts_us: 0,
            session: info.salt_a,
            frame_pos: pos,
            frame_count: frames as u16,
        };
        let pcm: Vec<u8> = (0..frames * info.channels as usize)
            .flat_map(|_| sample.to_le_bytes())
            .collect();
        let mut datagram = Vec::new();
        packet::encode_audio(&header, &[], &mut datagram);
        let encrypted = seal(
            &derive_key("secret", info.salt_a, info.salt_b),
            nonce_salt(info.salt_a, info.salt_b),
            seq,
            &datagram,
            &pcm,
        );
        packet::encode_audio(&header, &encrypted, &mut datagram);
        source.send_to(&datagram, destination).unwrap();
    };
    let mut forged = Vec::new();
    for flags in [0, 1] {
        let h = packet::AudioHeader {
            flags,
            seq: 99_000_000,
            ts_us: 0,
            session: info.salt_a,
            frame_pos: 0,
            frame_count: 1,
        };
        packet::encode_audio(&h, &vec![0; if flags == 1 { 20 } else { 4 }], &mut forged);
        source.send_to(&forged, destination).unwrap();
    }
    let overflow = packet::AudioHeader {
        flags: 1,
        seq: 99_000_000,
        ts_us: 0,
        session: info.salt_a,
        frame_pos: u64::MAX,
        frame_count: 1,
    };
    packet::encode_audio(&overflow, &[], &mut forged);
    let cipher = seal(
        &derive_key("secret", info.salt_a, info.salt_b),
        nonce_salt(info.salt_a, info.salt_b),
        overflow.seq,
        &forged,
        &[0; 4],
    );
    packet::encode_audio(&overflow, &cipher, &mut forged);
    source.send_to(&forged, destination).unwrap();
    send(info, 1_000_000, 1_000_000, 100, 16000);
    // Missing seq 1_000_001 contained 37 stereo frames. Later packet's sample
    // position preserves those frames despite the populated playback ring.
    send(info, 1_000_002, 1_000_137, 100, 16000);
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while consumer.len() < 460 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut old_audio = Vec::new();
    while let Some(sample) = consumer.pop() {
        old_audio.push(sample);
    }
    assert!(
        old_audio.len() >= 460,
        "gap duration remains in the output timeline: {}",
        old_audio.len()
    );
    assert!(
        old_audio.iter().filter(|&&s| s.abs() < 0.01).count() >= 60,
        "lost packet creates silence"
    );
    let new = packet::StreamInfo {
        sample_rate: 44100,
        channels: 1,
        salt_a: 33,
        ..info
    };
    let mut datagram = Vec::new();
    packet::encode_authenticated_info(&new, "secret", &mut datagram);
    source.send_to(&datagram, destination).unwrap();
    send(new, 0, 0, 100, -16000);
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while consumer.len() < 190 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    running.store(false, Ordering::Relaxed);
    worker.join().unwrap();
    let mut new_audio = Vec::new();
    while let Some(sample) = consumer.pop() {
        new_audio.push(sample);
    }
    assert!(
        new_audio.iter().any(|&s| s < -0.4),
        "new mono/rate session is converted with new keys"
    );
    assert_eq!(new_audio.len() % 2, 0);
    assert_eq!(metrics.auth_failures.load(Ordering::Relaxed), 3);
    assert_eq!(metrics.lost_frames.load(Ordering::Relaxed), 37);
    assert_eq!(metrics.reconnects.load(Ordering::Relaxed), 1);
    assert_eq!(
        metrics.wire_format.load(Ordering::Relaxed),
        (44100 << 16) | 1
    );
    assert_eq!(metrics.received_packets.load(Ordering::Relaxed), 3);
}

#[test]
fn public_subscribe_api_authenticates_metadata_over_ipv6_or_ipv4_loopback() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let mut source = UdpSource::bind(0, 48000, 2, "secret".into()).unwrap();
    let address = source.local_addr();
    let polling = Arc::new(AtomicBool::new(true));
    let polling_worker = polling.clone();
    let worker = std::thread::spawn(move || {
        while polling_worker.load(Ordering::Relaxed) {
            source.poll_subscribe();
            std::thread::sleep(Duration::from_millis(1));
        }
        source.poll_subscribe();
        source
    });
    assert!(super::sink::subscribe(&address, 10, "wrong", Duration::from_millis(10)).is_err());
    let subscribed =
        super::sink::subscribe(&address, 11, "secret", Duration::from_millis(100)).unwrap();
    assert_eq!(subscribed.info.flags, 1);
    assert_eq!(subscribed.info.salt_b, 11);
    polling.store(false, Ordering::Relaxed);
    assert!(worker.join().unwrap().has_peer());
}

#[test]
fn subscribe_cancellation_interrupts_long_handshake_promptly() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let silent_source = UdpSocket::bind("127.0.0.1:0").unwrap();
    let address = silent_source.local_addr().unwrap().to_string();
    let running = Arc::new(AtomicBool::new(true));
    let worker_running = running.clone();
    let started = std::time::Instant::now();
    let worker = std::thread::spawn(move || {
        super::sink::subscribe_cancellable(
            &address,
            5,
            "secret",
            Duration::from_secs(5),
            &worker_running,
        )
    });
    std::thread::sleep(Duration::from_millis(20));
    running.store(false, Ordering::Release);
    let result = worker.join().unwrap();
    assert_eq!(
        result.err().unwrap().kind(),
        std::io::ErrorKind::Interrupted
    );
    assert!(started.elapsed() < Duration::from_millis(300));
}

#[test]
fn abandoned_successful_subscription_releases_peer_before_lease_expires() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let mut source = UdpSource::bind(0, 48000, 2, "secret".into()).unwrap();
    let address = source.local_addr();
    let polling = Arc::new(AtomicBool::new(true));
    let worker_polling = polling.clone();
    let worker = std::thread::spawn(move || {
        while worker_polling.load(Ordering::Relaxed) {
            source.poll_subscribe();
            std::thread::sleep(Duration::from_millis(1));
        }
        source.poll_subscribe();
        source
    });
    let first = super::sink::subscribe(&address, 11, "secret", Duration::from_millis(100)).unwrap();
    // Mirrors any output-device negotiation/construction error after handshake.
    drop(first);
    let second =
        super::sink::subscribe(&address, 12, "secret", Duration::from_millis(100)).unwrap();
    drop(second);
    polling.store(false, Ordering::Relaxed);
    assert!(!worker.join().unwrap().has_peer());
}
