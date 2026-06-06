//! End-to-end UDP test over localhost: a UdpSource and a subscribing socket
//! exchange the handshake and one audio frame.

use super::{packet, source::UdpSource};
use std::net::UdpSocket;
use std::time::Duration;

#[test]
fn subscribe_then_receive_one_audio_frame() {
    let mut src = UdpSource::bind(0, 48000, 2).expect("bind");
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
