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

#[cfg(test)]
mod tests {
    use super::*;

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
