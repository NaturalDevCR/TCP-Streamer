# Phase 1 — Part C: Transport Extraction + Reliability — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract networking into a `transport` module behind a `Connection` trait (preparing Phase 2's TLS/multi-client without implementing them), remove the 1.5 s blocking HTTP handshake, consolidate the two competing reconnection mechanisms into one that honors `auto_reconnect`, and relocate the remaining orchestration out of the monolithic `manager.rs` into `engine/mod.rs`.

**Architecture:** A lean `Connection: Write + Send` trait abstracts a client/server TCP socket (with a best-effort `rtt()` and graceful `close()`); `transport::tcp_client::connect()` and `transport::tcp_server` own socket setup. HTTP detection becomes pure, testable helpers and a bounded non-blocking handshake. The network thread loops over a single reconnection policy. The orchestrator moves to `engine::run`, slimming `manager.rs` and removing the now-unused `stream.rs` enum.

**Tech Stack:** Rust (Tauri 2), `socket2 0.5`, `std::net`.

**Spec:** `docs/superpowers/specs/2026-06-05-fase1-estabilizacion-design.md` §5.6, §5.7, §4.
**Depends on:** Parts A and B complete.

> **FILE LOCATION — read first:** The orchestrator `start_audio_stream`, the network thread, and the send loop live in **`src-tauri/src/audio/manager.rs`** (~1012 lines), which imports the `StreamSocket` enum from the 7-line `stream.rs`. In M2–M4, "edit the server accept block / client branch / send loop" means **`manager.rs`**. In M5 the orchestrator function is _moved out of_ `manager.rs` into `engine/mod.rs`, and the `stream.rs` enum file is deleted. Anchor edits by the quoted code/comment — line numbers shifted in A and B.

---

## File Structure

| File                                          | Responsibility                                                                                                                                       | Status |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| `src-tauri/src/audio/transport/mod.rs`        | `Connection` trait + `pub mod tcp_client/tcp_server`                                                                                                 | Modify |
| `src-tauri/src/audio/transport/tcp_client.rs` | client connect + socket options (keepalive, nodelay, DSCP)                                                                                           | Create |
| `src-tauri/src/audio/transport/tcp_server.rs` | listener, **(pure)** HTTP helpers, non-blocking handshake                                                                                            | Create |
| `src-tauri/src/audio/engine/mod.rs`           | `pub fn run(...)` orchestrator (was `start_audio_stream`)                                                                                            | Modify |
| `src-tauri/src/audio/stream.rs`               | only the `StreamSocket` enum; **deleted** once the `Connection` trait replaces it                                                                    | Delete |
| `src-tauri/src/audio/manager.rs`              | **hosts the orchestrator + send loop**; receives the M2–M4 edits (handshake, client connect, reconnection); orchestrator then moves to `engine::run` | Modify |
| `src-tauri/src/audio/mod.rs`                  | drop `pub mod stream;`                                                                                                                               | Modify |

---

## Milestone 1 — `Connection` trait

### Task 1.1: Define the transport abstraction

**Files:**

- Modify: `src-tauri/src/audio/transport/mod.rs`

- [ ] **Step 1: Add the trait and a TCP implementation**

Replace `src-tauri/src/audio/transport/mod.rs` with:

```rust
//! Network transport abstraction.
//!
//! `Connection` is the seam the audio send-loop writes through. Today the only
//! implementation is TCP; Phase 2 adds TLS and multi-client behind the same
//! trait without touching the engine.

pub mod dscp;
pub mod tcp_client;
pub mod tcp_server;

use crate::audio::metrics::RttSample;
use std::io::Write;
use std::net::{Shutdown, TcpStream};

/// A writable, RTT-observable stream connection.
pub trait Connection: Write + Send {
    /// Best-effort smoothed RTT for metrics; `None` when unavailable.
    fn rtt(&self) -> Option<RttSample>;
    /// Human-readable peer description for logs.
    fn peer(&self) -> String;
    /// Graceful shutdown (sends TCP FIN). Idempotent / best-effort.
    fn close(&mut self);
}

/// TCP implementation of [`Connection`].
pub struct TcpConnection {
    pub stream: TcpStream,
    peer: String,
}

impl TcpConnection {
    pub fn new(stream: TcpStream) -> Self {
        let peer = stream
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        Self { stream, peer }
    }
}

impl Write for TcpConnection {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

impl Connection for TcpConnection {
    fn rtt(&self) -> Option<RttSample> {
        crate::audio::metrics::tcp_rtt(&self.stream)
    }
    fn peer(&self) -> String {
        self.peer.clone()
    }
    fn close(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}
```

> Create empty `tcp_client.rs` / `tcp_server.rs` placeholders (`// filled in next tasks`) so the crate compiles after this step.

- [ ] **Step 2: Verify compile**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: compiles (the engine still uses the old `StreamSocket`; that is fine — the trait is additive until M5).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/transport/
git commit -m "feat(transport): add Connection trait with TCP implementation"
```

---

## Milestone 2 — `tcp_server.rs`: pure HTTP helpers + non-blocking handshake

### Task 2.1: Pure HTTP helpers (TDD)

**Files:**

- Modify: `src-tauri/src/audio/transport/tcp_server.rs`

- [ ] **Step 1: Write the helpers + tests**

Put in `src-tauri/src/audio/transport/tcp_server.rs`:

```rust
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
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml tcp_server 2>&1 | tail -10`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/transport/tcp_server.rs
git commit -m "feat(transport): tested HTTP detection + header helpers"
```

### Task 2.2: Replace the blocking handshake in the accept path

**Files:**

- Modify: `src-tauri/src/audio/manager.rs` (the server accept block; relocated in M5)

- [ ] **Step 1: Swap the 1.5 s read-timeout handshake for a bounded non-blocking one**

In the server-mode accept branch, locate the block that currently does
`stream.set_read_timeout(Some(Duration::from_millis(1500)))`, the `stream.peek(&mut buf)` HTTP check, the inline header `format!(...)`, and `stream.set_read_timeout(None)`. Replace that whole region with:

```rust
                                let _ = stream.set_nodelay(true);

                                // DSCP parity (from Part A).
                                let tos_val = super::transport::dscp::dscp_to_tos(&dscp_clone);
                                if tos_val > 0 {
                                    let sref = socket2::SockRef::from(&stream);
                                    let _ = sref.set_tos(tos_val);
                                }

                                // Bounded, non-blocking handshake (no thread stall).
                                stream.set_nonblocking(true).ok();
                                let mut peekbuf = [0u8; 8];
                                let mut is_http = false;
                                let deadline = Instant::now() + Duration::from_millis(50);
                                loop {
                                    match stream.peek(&mut peekbuf) {
                                        Ok(n) if n >= 4 => {
                                            is_http = super::transport::tcp_server::looks_like_http(&peekbuf[..n]);
                                            break;
                                        }
                                        Ok(_) => {}
                                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                                        Err(_) => break,
                                    }
                                    if Instant::now() >= deadline {
                                        break;
                                    }
                                    thread::sleep(Duration::from_millis(2));
                                }
                                stream.set_nonblocking(false).ok();

                                if is_http {
                                    let mut hdr = [0u8; 1024];
                                    let _ = stream.read(&mut hdr); // consume request line (best-effort)
                                    let resp = super::transport::tcp_server::http_stream_headers("audio/wav");
                                    if let Err(e) = stream.write_all(resp.as_bytes()) {
                                        emit_log(&app_handle_net, "error", format!("Failed to send HTTP headers: {}", e));
                                    } else {
                                        emit_log(&app_handle_net, "info", "HTTP client detected. Serving audio/wav".to_string());
                                    }
                                }

                                let request_format = if is_http { "wav" } else { format_clone.as_str() };
                                emit_log(&app_handle_net, "success", format!("Client connected from {} (Format: {})", addr, request_format));
                                use_chunked = is_http;
                                wav_header_sent = false;
                                current_stream = Some(StreamSocket::Tcp(stream));
                                disconnect_time = None;
                                last_heartbeat = Instant::now();
```

> This assumes `dscp_clone` exists (added in Part A Task 1.3). Keep the surrounding `match l.accept()` arms (`Ok((mut stream, addr))`, `WouldBlock`, error) intact.

- [ ] **Step 2: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8`
Expected: compiles; all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/manager.rs
git commit -m "fix(transport): bounded non-blocking HTTP handshake (removes 1.5s stall)"
```

---

## Milestone 3 — `tcp_client.rs`: extract client connect

### Task 3.1: Move client socket setup into a function

**Files:**

- Modify: `src-tauri/src/audio/transport/tcp_client.rs`
- Modify: `src-tauri/src/audio/manager.rs`

- [ ] **Step 1: Write `connect` in `tcp_client.rs`**

Put in `src-tauri/src/audio/transport/tcp_client.rs` (this consolidates the existing client socket setup: socket2 creation, send-buffer size, nodelay, keepalive, DSCP, `connect_timeout`):

```rust
//! TCP client connection setup.

use socket2::{Domain, Protocol, Socket, TcpKeepalive, Type};
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// Connects to `ip:port` with audio-friendly socket options and the requested
/// DSCP marking. Returns a ready-to-write blocking `TcpStream`.
pub fn connect(ip: &str, port: u16, dscp_strategy: &str, connect_timeout: Duration) -> io::Result<TcpStream> {
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
    let _ = socket.set_send_buffer_size(32 * 1024);
    let _ = socket.set_nodelay(true);
    let keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(10))
        .with_interval(Duration::from_secs(1));
    let _ = socket.set_tcp_keepalive(&keepalive);

    let tos = super::dscp::dscp_to_tos(dscp_strategy);
    if tos > 0 {
        let _ = socket.set_tos(tos);
    }

    let addr: SocketAddr = format!("{ip}:{port}")
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("invalid address: {e}")))?;
    socket.connect_timeout(&addr.into(), connect_timeout)?;
    Ok(socket.into())
}
```

- [ ] **Step 2: Use it in the client branch of `manager.rs`**

In the client-mode branch, replace the inline `Socket::new(...)` setup + DSCP + `connect_timeout` logic with a call to `connect`, keeping the existing backoff/retry around it:

```rust
                    match super::transport::tcp_client::connect(
                        &ip_clone, port, &dscp_clone, Duration::from_secs(2),
                    ) {
                        Ok(stream) => {
                            emit_log(&app_handle_net, "success", format!("Connected to {}:{}", ip_clone, port));
                            current_stream = Some(StreamSocket::Tcp(stream));
                            disconnect_time = None;
                            retry_delay = Duration::from_secs(2);
                            last_heartbeat = Instant::now();
                        }
                        Err(e) => {
                            emit_log(&app_handle_net, "error", format!("Connection failed: {}", e));
                            thread::sleep(add_jitter(retry_delay));
                            retry_delay = (retry_delay * 2).min(MAX_RETRY_DELAY);
                        }
                    }
```

- [ ] **Step 3: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8`
Expected: compiles; all tests pass. Remove now-unused `socket2` imports from `manager.rs` if the compiler flags them.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/transport/tcp_client.rs src-tauri/src/audio/manager.rs
git commit -m "refactor(transport): extract client connect with socket options"
```

---

## Milestone 4 — Consolidate reconnection

### Task 4.1: Thread `auto_reconnect` into the engine and honor it

**Files:**

- Modify: `src-tauri/src/audio/manager.rs`
- Modify: `src-tauri/src/audio/manager.rs`

- [ ] **Step 1: Pass `auto_reconnect` into `start_audio_stream`**

Add `auto_reconnect: bool,` to the `start_audio_stream` signature (after `is_server`), clone it for the network thread (`let auto_reconnect_net = auto_reconnect;`), and update both call sites in `manager.rs` (the initial Start and — until Task 4.2 removes it — the reconnect call) to pass it.

- [ ] **Step 2: Honor it on client disconnect**

In the send path, find where a write error disconnects the client (the branch that calls `close_tcp_stream(s, "write error", ...)` then sets `current_stream = None`). After setting `current_stream = None`, add:

```rust
                         if !is_server_clone && !auto_reconnect_net {
                             emit_log(&app_handle_net, "info", "Auto-reconnect disabled; stopping stream.".to_string());
                             is_running_clone.store(false, Ordering::Relaxed);
                         }
```

> Server mode keeps listening for the next client regardless; only client mode stops when auto-reconnect is off.

- [ ] **Step 3: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8`
Expected: compiles; all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/manager.rs
git commit -m "feat(audio): single reconnection policy honoring auto_reconnect (engine-side)"
```

### Task 4.2: Remove the manager's redundant reconnection

**Files:**

- Modify: `src-tauri/src/audio/manager.rs`

- [ ] **Step 1: Delete the manager-level reconnect machinery**

In `manager.rs`'s `AudioState::new` thread loop, remove:

- the `should_reconnect` `AtomicBool` and all its `store`/`load` uses,
- the `current_params: Option<StreamParams>` binding and the `StreamParams` struct,
- the entire reconnection block (`if should_reconnect.load(...) && current_stream_handle.is_none() { ... attempt reconnect ... } else if ... { thread::sleep(200ms) }`).

Replace the loop body so it only: handles `Start` (stop previous, start new), `Stop` (stop), `Empty` (`thread::sleep(Duration::from_millis(200))`), `Disconnected` (`break`). The network thread now owns reconnection.

- [ ] **Step 2: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8`
Expected: compiles (no unused `StreamParams`/`should_reconnect` warnings); all tests pass.

- [ ] **Step 3: Manual check — auto-reconnect both ways**

Run `npm run tauri dev`; client mode against a Snapserver:

- With **Auto-Reconnect ON**: kill the server → logs show retry/backoff → restart server → reconnects.
- With **Auto-Reconnect OFF**: kill the server → stream stops cleanly (one "stopping" log), no retry storm.

Expected: exactly one reconnection mechanism, behaving per the toggle.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/manager.rs
git commit -m "refactor(audio): remove redundant manager-level reconnection"
```

---

## Milestone 5 — Relocate orchestration into `engine/mod.rs`

> This is a structural move guided by the interfaces established above. It changes no behavior; the full test suite + the smoke test are the gate. Do it in the listed sub-steps, compiling after each.

### Task 5.1: Route the send loop through the `Connection` trait

**Files:**

- Modify: `src-tauri/src/audio/manager.rs`

- [ ] **Step 1: Replace `StreamSocket` with `Box<dyn Connection>`**

In the network thread, change `let mut current_stream: Option<StreamSocket> = None;` to `let mut current_stream: Option<Box<dyn super::transport::Connection>> = None;`. At each connection point, wrap the `TcpStream` as `Some(Box::new(super::transport::TcpConnection::new(stream)))` instead of `Some(StreamSocket::Tcp(stream))`.

- [ ] **Step 2: Update the write/close/RTT sites to use the trait**

- Writes: the `ChunkedWriter::new(stream)` / `stream.write_all(&payload)` sites take `&mut **conn` (a `&mut dyn Write`). `ChunkedWriter::new(conn.as_mut())` works since `Box<dyn Connection>: Write`.
- Disconnect: replace `close_tcp_stream(s, ...)` with `conn.close();`.
- RTT (from Part B): replace the `let StreamSocket::Tcp(tcp) = s;` match with `conn.rtt()`.

- [ ] **Step 3: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8`
Expected: compiles; all tests pass. The `StreamSocket` enum and `close_tcp_stream` helper are now unused.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/manager.rs
git commit -m "refactor(engine): send loop writes through Connection trait"
```

### Task 5.2: Move the orchestrator to `engine/mod.rs` and delete `stream.rs`

**Files:**

- Modify: `src-tauri/src/audio/engine/mod.rs`
- Delete: `src-tauri/src/audio/stream.rs`
- Modify: `src-tauri/src/audio/mod.rs`
- Modify: `src-tauri/src/audio/manager.rs`

- [ ] **Step 1: Relocate the function**

Move `start_audio_stream` (and any remaining private helpers it still uses, e.g. `add_jitter`) from `manager.rs` into `src-tauri/src/audio/engine/mod.rs`, renaming it `pub fn run(...)` with the same parameter list. Update its internal `super::` paths: it now lives in `engine`, so `super::transport::…`, `super::metrics::…`, `super::constants::…`, `super::stats::…` resolve from `audio::engine`. The `StreamSocket` enum and `close_tcp_stream` are dropped (replaced in Task 5.1).

- [ ] **Step 2: Delete `stream.rs` and update module wiring**

- Delete `src-tauri/src/audio/stream.rs`.
- In `src-tauri/src/audio/mod.rs`, remove `pub mod stream;`.
- In `manager.rs`, delete the relocated `start_audio_stream`, `close_tcp_stream`, and `add_jitter` definitions and the `use super::stream::StreamSocket;` import; change the (single, post-Task-4.2) call site to `super::engine::run(...)`.

- [ ] **Step 3: Verify compile + full suite**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml && \
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: compiles clean; all tests pass; clippy clean. No reference to `stream.rs` remains (`grep -rn "audio::stream\|mod stream" src-tauri/src` → empty).

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor(engine): move orchestrator from manager.rs to engine::run; delete stream.rs enum"
```

---

## Milestone 6 — Verification

### Task 6.1: Full green gate + reliability smoke test

- [ ] **Step 1: Complete check suite**

Run:

```bash
npm test && npm run typecheck && npm run lint && \
cargo test --manifest-path src-tauri/Cargo.toml && \
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: all green (Rust tests up by ~3 for the server helpers).

- [ ] **Step 2: Behavioural verification**

Run `npm run tauri dev` and confirm, with no regressions vs Part B:

1. **HTTP handshake:** open `http://<LAN_IP>:<port>/stream.wav` in a browser — audio starts within ~tens of ms (no multi-second pause); a raw-PCM client (Snapserver `mode=client`) still gets raw PCM.
2. **Reconnection:** client mode, toggle Auto-Reconnect both ways and kill/restart the server (as in Task 4.2 Step 3) — one mechanism, correct behavior.
3. **DSCP/RTT/adaptive** (from A/B) still work.

Expected: reliability fixes hold; the app behaves identically to before except faster HTTP start and cleaner reconnection.

- [ ] **Step 3: Final commit (if needed)**

```bash
git add -A && git commit -m "chore: Phase 1 Part C verification fixes"
```

---

## Self-Review (author)

- **Spec coverage:** §5.7 non-blocking handshake → M2; §5.6 reconnection consolidation → M4; §4 transport seam + engine orchestration → M1 + M3 + M5. The `Connection` trait is defined and used but no new transports are added (TLS/multi-client remain Phase 2, per spec §3.2).
- **Type consistency:** `Connection: Write + Send { rtt()->Option<RttSample>; peer()->String; close() }`; `TcpConnection::new(TcpStream)`; `tcp_client::connect(&str,u16,&str,Duration)->io::Result<TcpStream>`; `tcp_server::{looks_like_http(&[u8])->bool, http_stream_headers(&str)->String}`. `RttSample` is the Part B type. `engine::run(...)` keeps `start_audio_stream`'s parameter list plus `auto_reconnect` (added in Task 4.1).
- **No placeholders:** the empty `tcp_client.rs`/`tcp_server.rs` created in M1 are explicitly filled in M2/M3 (sequenced), not left blank.
- **Risk note:** M5 is the one large structural move; it is split into trait-routing (5.1) then relocation (5.2), each compiling independently, with clippy + the full suite + the smoke test as gates. If `Box<dyn Connection>` causes borrow friction at the `ChunkedWriter` site, write through `conn.as_mut()` (a `&mut dyn Connection`, which is `Write`).

## Phase 1 complete after this plan

With Parts A + B + C merged, Phase 1's acceptance criteria (spec §8) are met: artifacts untracked, RT-safe callback, working DSCP, real adaptive buffer, effective Buffer Size, honest metrics, single reconnection, non-blocking handshake, decomposed + tested engine. Next: **Phase 2** (TLS + auth, multi-client, IPv6/hostnames) and **Phase 3** (UI/UX redesign), each with its own spec.
