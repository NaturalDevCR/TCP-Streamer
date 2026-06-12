//! Honest streaming metrics: real glitch counters, quality scoring, and
//! best-effort TCP round-trip time. Replaces the previous score derived from
//! socket `write()` timing.

use std::net::TcpStream;

/// A best-effort RTT reading from the OS TCP stack.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RttSample {
    pub srtt_ms: f32,
    pub rttvar_ms: f32,
}

/// Computes a 0-100 stream-quality score (higher is better) from real signals.
///
/// - `glitches_delta`: underruns + overruns observed in the last window — the
///   only thing the listener actually hears; weighted heaviest.
/// - `occupancy_ratio`: buffer fill 0.0..=1.0 — high occupancy means latency.
/// - `rtt_ms`: best-effort network RTT, or `None` when unavailable.
pub fn quality_score(glitches_delta: u64, occupancy_ratio: f32, rtt_ms: Option<f32>) -> u8 {
    let mut score: i32 = 100;

    // Audible glitches: each one in the window is a large penalty (cap the hit).
    score -= (glitches_delta.min(10) as i32) * 8; // up to -80

    // Latency from over-buffering (only the half above 50% counts).
    let occ = occupancy_ratio.clamp(0.0, 1.0);
    if occ > 0.5 {
        score -= ((occ - 0.5) * 2.0 * 20.0) as i32; // up to -20
    }

    // Network RTT above 50 ms erodes the score, capped.
    if let Some(rtt) = rtt_ms {
        if rtt > 50.0 {
            score -= ((rtt - 50.0) / 10.0).min(20.0) as i32; // up to -20
        }
    }

    score.clamp(0, 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_stream_scores_100() {
        assert_eq!(quality_score(0, 0.1, Some(5.0)), 100);
        assert_eq!(quality_score(0, 0.1, None), 100);
    }

    #[test]
    fn glitches_dominate_the_penalty() {
        assert_eq!(quality_score(1, 0.0, None), 92);
        assert_eq!(quality_score(5, 0.0, None), 60);
        assert_eq!(quality_score(100, 0.0, None), 20); // capped at 10 glitches => -80
    }

    #[test]
    fn high_occupancy_adds_latency_penalty() {
        // occupancy 1.0 => (1.0-0.5)*2*20 = 20 penalty
        assert_eq!(quality_score(0, 1.0, None), 80);
    }

    #[test]
    fn high_rtt_erodes_score_and_is_capped() {
        assert_eq!(quality_score(0, 0.0, Some(150.0)), 90); // (150-50)/10 = 10
        assert_eq!(quality_score(0, 0.0, Some(5000.0)), 80); // capped at 20
    }

    #[test]
    fn score_never_underflows() {
        assert_eq!(quality_score(100, 1.0, Some(5000.0)), 0);
    }
}

/// Reads a best-effort RTT sample from the OS TCP stack. Returns `None` when
/// the platform does not expose it or the syscall fails.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn tcp_rtt(stream: &TcpStream) -> Option<RttSample> {
    use std::os::unix::io::AsRawFd;
    let fd = stream.as_raw_fd();
    let mut info: libc::tcp_info = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::tcp_info>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::IPPROTO_TCP,
            libc::TCP_INFO,
            &mut info as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if rc != 0 {
        return None;
    }
    // Linux reports these in microseconds.
    Some(RttSample {
        srtt_ms: info.tcpi_rtt as f32 / 1000.0,
        rttvar_ms: info.tcpi_rttvar as f32 / 1000.0,
    })
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn tcp_rtt(stream: &TcpStream) -> Option<RttSample> {
    use std::os::unix::io::AsRawFd;
    let fd = stream.as_raw_fd();
    let mut info: libc::tcp_connection_info = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::tcp_connection_info>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::IPPROTO_TCP,
            libc::TCP_CONNECTION_INFO,
            &mut info as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if rc != 0 {
        return None;
    }
    // macOS reports these in milliseconds.
    Some(RttSample {
        srtt_ms: info.tcpi_srtt as f32,
        rttvar_ms: info.tcpi_rttvar as f32,
    })
}

/// Fallback for platforms where we have not yet validated a TCP_INFO reader
/// (e.g. Windows SIO_TCP_INFO). RTT is reported as unavailable ("n/a").
#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios"
)))]
pub fn tcp_rtt(_stream: &TcpStream) -> Option<RttSample> {
    None
}

#[cfg(all(test, unix))]
mod rtt_tests {
    use super::*;
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};

    #[test]
    fn tcp_rtt_returns_some_on_an_established_loopback_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().unwrap();
        let mut client = TcpStream::connect(addr).expect("connect");
        let (mut server, _) = listener.accept().expect("accept");
        // Exchange a little data so the stack has an RTT estimate.
        client.write_all(b"ping").ok();
        server.write_all(b"pong").ok();
        // On an established connection the OS should return a sample.
        let sample = tcp_rtt(&client);
        assert!(sample.is_some(), "expected an RTT sample on loopback");
        let s = sample.unwrap();
        assert!(
            s.srtt_ms >= 0.0 && s.srtt_ms < 1000.0,
            "loopback srtt sane: {}",
            s.srtt_ms
        );
    }
}
