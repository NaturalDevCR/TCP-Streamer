//! TCP server helpers: HTTP detection and response headers (pure), plus the
//! non-blocking accept handshake used by the engine.

/// True if the peeked bytes begin an HTTP request we should serve as audio/wav.
pub fn looks_like_http(peeked: &[u8]) -> bool {
    peeked.starts_with(b"GET ") || peeked.starts_with(b"HEAD ")
}

/// Builds chunked-transfer HTTP response headers for a live audio stream.
pub fn http_stream_headers(content_type: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\n\
Content-Type: {content_type}\r\n\
Transfer-Encoding: chunked\r\n\
Connection: keep-alive\r\n\
Cache-Control: no-cache, no-store, must-revalidate\r\n\
Access-Control-Allow-Origin: *\r\n\r\n"
    )
}

#[derive(Debug, PartialEq, Eq)]
pub enum Handshake {
    Raw,
    HttpGet,
    HttpHead,
}

/// Inspect a raw receiver or consume a bounded complete HTTP request. Partial
/// request prefixes keep their HTTP interpretation until the header deadline.
pub fn inspect_request(
    stream: &mut std::net::TcpStream,
    running: &std::sync::atomic::AtomicBool,
) -> std::io::Result<Handshake> {
    use std::io::{self, Read};
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};
    stream.set_nonblocking(true)?;
    let start = Instant::now();
    let mut peek = [0u8; 8];
    loop {
        if !running.load(Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Handshake cancelled",
            ));
        }
        match stream.peek(&mut peek) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Receiver disconnected",
                ))
            }
            Ok(n) if looks_like_http(&peek[..n]) => break,
            Ok(n) if !(b"GET ".starts_with(&peek[..n]) || b"HEAD ".starts_with(&peek[..n])) => {
                stream.set_nonblocking(false)?;
                return Ok(Handshake::Raw);
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }
        if start.elapsed() >= Duration::from_millis(100) {
            if peek[0] == 0 {
                stream.set_nonblocking(false)?;
                return Ok(Handshake::Raw);
            }
            if start.elapsed() >= Duration::from_secs(1) {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Incomplete HTTP prefix",
                ));
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut headers = Vec::with_capacity(1024);
    let mut buf = [0u8; 512];
    loop {
        if !running.load(Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Handshake cancelled",
            ));
        }
        if start.elapsed() >= Duration::from_secs(1) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "HTTP headers timed out",
            ));
        }
        match stream.read(&mut buf) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Incomplete HTTP request",
                ))
            }
            Ok(n) => headers.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }
        if headers.len() > 8192 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HTTP headers exceed 8KiB",
            ));
        }
        if headers.windows(4).any(|w| w == b"\r\n\r\n") {
            stream.set_nonblocking(false)?;
            return Ok(if headers.starts_with(b"HEAD ") {
                Handshake::HttpHead
            } else {
                Handshake::HttpGet
            });
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragmented_headers_preserve_get_and_head_and_are_consumed() {
        use std::io::Write;
        for (method, expected) in [("GET", Handshake::HttpGet), ("HEAD", Handshake::HttpHead)] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let mut client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (mut server, _) = listener.accept().unwrap();
            let request = format!("{method} /stream.wav HTTP/1.1\r\nHost: localhost\r\n\r\n");
            let worker = std::thread::spawn(move || {
                for chunk in request.as_bytes().chunks(2) {
                    client.write_all(chunk).unwrap();
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                client
            });
            assert_eq!(
                inspect_request(&mut server, &std::sync::atomic::AtomicBool::new(true)).unwrap(),
                expected
            );
            let _client = worker.join().unwrap();
            server.set_nonblocking(true).unwrap();
            assert_eq!(
                server.peek(&mut [0u8; 1]).unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
        }
    }
    #[test]
    fn detects_http_get_and_head() {
        assert!(looks_like_http(b"GET /stream.wav HTTP/1.1"));
        assert!(looks_like_http(b"HEAD / HTTP/1.1"));
    }

    #[test]
    fn rejects_raw_pcm_and_partial_bytes() {
        assert!(!looks_like_http(&[0x00, 0x01, 0x02, 0x03]));
        assert!(!looks_like_http(b"GE")); // too short to be sure
        assert!(!looks_like_http(b""));
    }

    #[test]
    fn headers_contain_content_type_and_chunked() {
        let h = http_stream_headers("audio/wav");
        assert!(h.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(h.contains("Content-Type: audio/wav\r\n"));
        assert!(h.contains("Transfer-Encoding: chunked\r\n"));
        assert!(h.ends_with("\r\n\r\n"));
    }
}
