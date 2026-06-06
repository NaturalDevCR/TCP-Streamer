# Phase 2B-i: Sink Mode + Native UDP (plaintext) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make tcp-streamer bidirectional — add a Sink role that receives a native UDP audio stream and plays it to a local output device, plus a Source "native UDP" sub-mode that streams to a subscriber. End-to-end tcp-streamer↔tcp-streamer audio over plaintext UDP.

**Architecture:** Pure, tested modules for the wire format (`transport/udp/packet.rs`), de-jitter (`transport/udp/jitter.rs`), and decoding (`engine/decoder.rs`); a real-time-safe cpal output path (`engine/playback.rs`) that pulls from a `FrameSource`; and UDP source/sink I/O (`transport/udp/{source,sink}.rs`). Reuses the Phase-1 capture→ring→encoder path on the source side and the Phase-2A latency profile for jitter depth.

**Tech Stack:** Rust (Tauri 2), `cpal 0.15.3` (output), `std::net::UdpSocket`; Vue 3 + Pinia + Vitest. **No new crates** (crypto/mDNS are Plans 2B-ii / 2B-iii).

**Spec:** `docs/superpowers/specs/2026-06-06-fase2b-modo-nativo-design.md`
**Depends on:** Phase 2A merged (latency profile, generic naming). **Anchor edits by quoted code.**

**Wire format (used across this plan):**
```
AUDIO       : magic[2]=b"TS" | ver[1]=1 | type[1]=0 | flags[1] | seq[8 BE] | ts_us[8 BE] | pcm...
SUBSCRIBE   : magic[2]=b"TS" | ver[1]=1 | type[1]=1 | salt_b[8]
STREAM_INFO : magic[2]=b"TS" | ver[1]=1 | type[1]=2 | sample_rate[4 BE] | channels[2 BE] | salt_a[8] | flags[1]
```
`flags` bit0 = encrypted (always 0 in 2B-i; the field exists so 2B-ii adds crypto without a format change).

---

## File Structure

| File | Responsibility | Status |
|---|---|---|
| `src-tauri/src/audio/engine/decoder.rs` | **(pure)** PCM i16 LE → f32 | Create |
| `src-tauri/src/audio/transport/udp/mod.rs` | declares udp submodules | Create |
| `src-tauri/src/audio/transport/udp/packet.rs` | **(pure)** wire format encode/decode | Create |
| `src-tauri/src/audio/transport/udp/jitter.rs` | **(pure)** reorder/de-jitter buffer | Create |
| `src-tauri/src/audio/engine/playback.rs` | RT-safe cpal output + `FrameSource` trait | Create |
| `src-tauri/src/audio/transport/udp/sink.rs` | sink I/O: subscribe, recv→jitter, FrameSource impl | Create |
| `src-tauri/src/audio/transport/udp/source.rs` | source I/O: accept subscribe, stream audio | Create |
| `src-tauri/src/audio/engine/sink.rs` | sink orchestrator (output stream + recv thread) | Create |
| `src-tauri/src/audio/{mod,engine/mod,transport/mod}.rs` | wire new modules | Modify |
| `src-tauri/src/audio/commands.rs` · `manager.rs` · `lib.rs` | `get_output_devices`, `start_sink`, `stop_sink`; native source option | Modify |
| `src/stores/settings.ts` · `stream.ts` · components | role + sink UI (output device, source addr) | Modify |

---

## Milestone 1 — `engine/decoder.rs` (pure, TDD)

### Task 1.1: PCM → f32 decoder

**Files:** Create `src-tauri/src/audio/engine/decoder.rs`; Modify `src-tauri/src/audio/engine/mod.rs`

- [ ] **Step 1: Declare the module**

In `src-tauri/src/audio/engine/mod.rs`, add `pub mod decoder;` to the `pub mod` block.

- [ ] **Step 2: Write the module + tests**

Create `src-tauri/src/audio/engine/decoder.rs`:

```rust
//! Decoding of signed 16-bit little-endian PCM into f32 samples. Inverse of
//! `engine::encoder::encode_f32_to_pcm_i16_le`.

/// Decodes i16 LE PCM bytes into f32 samples in [-1.0, 1.0], replacing `out`.
/// A trailing odd byte (incomplete sample) is ignored.
pub fn decode_pcm_i16_le_to_f32(bytes: &[u8], out: &mut Vec<f32>) {
    out.clear();
    out.reserve(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let v = i16::from_le_bytes([pair[0], pair[1]]);
        out.push(v as f32 / 32768.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::engine::encoder::encode_f32_to_pcm_i16_le;

    #[test]
    fn decodes_known_values() {
        let mut out = Vec::new();
        // 0x0000 -> 0.0 ; 0x7FFF -> ~1.0 ; 0x8001 (-32767) -> ~-1.0
        decode_pcm_i16_le_to_f32(&[0x00, 0x00, 0xFF, 0x7F, 0x01, 0x80], &mut out);
        assert_eq!(out.len(), 3);
        assert!((out[0]).abs() < 1e-6);
        assert!((out[1] - 0.999).abs() < 0.01);
        assert!((out[2] + 0.999).abs() < 0.01);
    }

    #[test]
    fn ignores_trailing_odd_byte() {
        let mut out = Vec::new();
        decode_pcm_i16_le_to_f32(&[0x00, 0x00, 0x11], &mut out);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn roundtrips_with_encoder() {
        let samples = [0.0f32, 0.5, -0.5, 0.25];
        let mut pcm = Vec::new();
        encode_f32_to_pcm_i16_le(&samples, &mut pcm);
        let mut back = Vec::new();
        decode_pcm_i16_le_to_f32(&pcm, &mut back);
        assert_eq!(back.len(), samples.len());
        for (a, b) in samples.iter().zip(back.iter()) {
            assert!((a - b).abs() < 1e-3, "{a} vs {b}");
        }
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml engine::decoder 2>&1 | tail -8`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/engine/mod.rs src-tauri/src/audio/engine/decoder.rs
git commit -m "feat(engine): add tested PCM i16 LE -> f32 decoder"
```

---

## Milestone 2 — `transport/udp/packet.rs` (pure, TDD)

### Task 2.1: Wire format encode/decode

**Files:** Create `src-tauri/src/audio/transport/udp/mod.rs`, `packet.rs`; Modify `src-tauri/src/audio/transport/mod.rs`

- [ ] **Step 1: Declare the modules**

In `src-tauri/src/audio/transport/mod.rs`, add `pub mod udp;`. Create `src-tauri/src/audio/transport/udp/mod.rs` with:
```rust
//! Native low-latency UDP transport (Phase 2B).
pub mod jitter;
pub mod packet;
```

- [ ] **Step 2: Write `packet.rs` + tests**

Create `src-tauri/src/audio/transport/udp/packet.rs`:

```rust
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
    if buf.len() < AUDIO_HEADER_LEN || buf[0..2] != MAGIC || buf[2] != VERSION || buf[3] != TYPE_AUDIO
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
        let h = AudioHeader { flags: 0, seq: 42, ts_us: 123456 };
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
        let info = StreamInfo { sample_rate: 48000, channels: 2, salt_a: 99, flags: 0 };
        let mut out = Vec::new();
        encode_stream_info(&info, &mut out);
        assert_eq!(decode_stream_info(&out), Some(info));
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml transport::udp::packet 2>&1 | tail -8`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/transport/mod.rs src-tauri/src/audio/transport/udp/
git commit -m "feat(udp): tested native wire format (audio + handshake)"
```

---

## Milestone 3 — `transport/udp/jitter.rs` (pure, TDD)

### Task 3.1: Reorder / de-jitter buffer

**Files:** Create `src-tauri/src/audio/transport/udp/jitter.rs`

- [ ] **Step 1: Write the module + tests**

Create `src-tauri/src/audio/transport/udp/jitter.rs`:

```rust
//! De-jitter / reorder buffer keyed by sequence number (pure, testable).
//!
//! The source numbers audio frames 0,1,2,... The sink inserts them (possibly
//! out of order) and pulls them in order for playback. A frame still missing
//! once `lost_after` later frames have arrived is declared lost (a gap → the
//! caller plays silence). Frames older than the next expected one are dropped.

use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
pub enum Pop {
    /// The next in-order frame's payload.
    Frame(Vec<u8>),
    /// The next frame is declared lost; play silence and advance.
    Gap,
    /// Not enough buffered yet; play silence but keep waiting.
    Starved,
}

pub struct JitterBuffer {
    next_seq: u64,
    buf: BTreeMap<u64, Vec<u8>>,
    lost_after: usize,
}

impl JitterBuffer {
    /// `lost_after` = how many later frames must be buffered before a missing
    /// `next_seq` is treated as lost. The stream starts at seq 0.
    pub fn new(lost_after: usize) -> Self {
        Self { next_seq: 0, buf: BTreeMap::new(), lost_after: lost_after.max(1) }
    }

    pub fn push(&mut self, seq: u64, frame: Vec<u8>) {
        if seq < self.next_seq {
            return; // too old
        }
        self.buf.insert(seq, frame);
    }

    pub fn pop(&mut self) -> Pop {
        if let Some(frame) = self.buf.remove(&self.next_seq) {
            self.next_seq += 1;
            return Pop::Frame(frame);
        }
        if self.buf.len() >= self.lost_after {
            self.next_seq += 1; // declare lost, skip the gap
            return Pop::Gap;
        }
        Pop::Starved
    }

    pub fn buffered(&self) -> usize {
        self.buf.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(b: u8) -> Vec<u8> {
        vec![b]
    }

    #[test]
    fn in_order() {
        let mut j = JitterBuffer::new(3);
        j.push(0, f(0));
        j.push(1, f(1));
        assert_eq!(j.pop(), Pop::Frame(f(0)));
        assert_eq!(j.pop(), Pop::Frame(f(1)));
        assert_eq!(j.pop(), Pop::Starved);
    }

    #[test]
    fn out_of_order_is_reordered() {
        let mut j = JitterBuffer::new(3);
        j.push(2, f(2));
        j.push(0, f(0));
        j.push(1, f(1));
        assert_eq!(j.pop(), Pop::Frame(f(0)));
        assert_eq!(j.pop(), Pop::Frame(f(1)));
        assert_eq!(j.pop(), Pop::Frame(f(2)));
    }

    #[test]
    fn missing_frame_becomes_gap_once_enough_buffered() {
        let mut j = JitterBuffer::new(2);
        // seq 0 never arrives; 1 and 2 do.
        j.push(1, f(1));
        j.push(2, f(2));
        assert_eq!(j.pop(), Pop::Gap); // 0 declared lost (2 buffered >= lost_after)
        assert_eq!(j.pop(), Pop::Frame(f(1)));
        assert_eq!(j.pop(), Pop::Frame(f(2)));
    }

    #[test]
    fn waits_when_not_enough_buffered() {
        let mut j = JitterBuffer::new(3);
        j.push(1, f(1)); // only 1 buffered, < lost_after
        assert_eq!(j.pop(), Pop::Starved); // still waiting for 0
        j.push(0, f(0)); // late but recovered
        assert_eq!(j.pop(), Pop::Frame(f(0)));
        assert_eq!(j.pop(), Pop::Frame(f(1)));
    }

    #[test]
    fn drops_stale_frames() {
        let mut j = JitterBuffer::new(2);
        j.push(0, f(0));
        assert_eq!(j.pop(), Pop::Frame(f(0))); // next_seq now 1
        j.push(0, f(9)); // stale
        assert_eq!(j.buffered(), 0);
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml transport::udp::jitter 2>&1 | tail -8`
Expected: 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/transport/udp/jitter.rs
git commit -m "feat(udp): tested de-jitter reorder buffer"
```

---

## Milestone 4 — `engine/playback.rs`: RT-safe output + `FrameSource`

### Task 4.1: FrameSource trait + cpal output builder

**Files:** Create `src-tauri/src/audio/engine/playback.rs`; Modify `src-tauri/src/audio/engine/mod.rs`

- [ ] **Step 1: Declare the module**

In `src-tauri/src/audio/engine/mod.rs`, add `pub mod playback;`.

- [ ] **Step 2: Write `playback.rs`**

Create `src-tauri/src/audio/engine/playback.rs`. The output callback is real-time safe: it holds no lock and allocates nothing; it pulls already-decoded f32 samples from a lock-free consumer that the receive thread fills.

```rust
//! Real-time-safe cpal output playback. The callback pulls f32 samples from a
//! lock-free ring consumer; underruns are filled with silence.

use cpal::traits::DeviceTrait;
use cpal::StreamConfig;
use ringbuf::HeapConsumer;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Builds an output stream that drains f32 samples from `consumer`. Missing
/// samples (underrun) are written as silence and counted in `underruns`.
pub fn build_output_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    mut consumer: HeapConsumer<f32>,
    underruns: Arc<AtomicU64>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let err_fn = |err: cpal::StreamError| log::error!("CPAL output error: {}", err);
    device.build_output_stream(
        config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let popped = consumer.pop_slice(data);
            if popped < data.len() {
                for s in &mut data[popped..] {
                    *s = 0.0;
                }
                underruns.fetch_add((data.len() - popped) as u64, Ordering::Relaxed);
            }
        },
        err_fn,
        None,
    )
}
```

- [ ] **Step 3: Verify compile**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/engine/mod.rs src-tauri/src/audio/engine/playback.rs
git commit -m "feat(engine): RT-safe cpal output playback path"
```

### Task 4.2: `get_output_devices` command

**Files:** Modify `src-tauri/src/audio/commands.rs`, `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the command**

In `commands.rs`, add (mirrors `get_input_devices` but for outputs, no loopback):

```rust
/// Enumerates available audio output devices across all CPAL hosts.
#[tauri::command]
pub fn get_output_devices() -> Result<Vec<String>, AudioError> {
    let mut all = Vec::new();
    for host_id in cpal::available_hosts() {
        if let Ok(host) = cpal::host_from_id(host_id) {
            if let Ok(devices) = host.output_devices() {
                for dev in devices {
                    if let Ok(name) = dev.name() {
                        all.push(name);
                    }
                }
            }
        }
    }
    all.sort();
    all.dedup();
    Ok(all)
}
```

Add `use cpal::traits::{DeviceTrait, HostTrait};` if not already imported (it is, from `get_input_devices`).

- [ ] **Step 2: Register it**

In `src-tauri/src/lib.rs`, add `audio::get_output_devices,` to the `tauri::generate_handler![...]` list.

- [ ] **Step 3: Verify compile**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/commands.rs src-tauri/src/lib.rs
git commit -m "feat(audio): get_output_devices command"
```

---

## Milestone 5 — Sink I/O + orchestrator

### Task 5.1: `transport/udp/sink.rs` — receive loop

**Files:** Create `src-tauri/src/audio/transport/udp/sink.rs`; Modify `src-tauri/src/audio/transport/udp/mod.rs`

- [ ] **Step 1: Declare the submodule**

In `transport/udp/mod.rs`, add `pub mod sink;` and `pub mod source;` (source created in M6; add a placeholder `// source in M6` file now so the crate compiles, replaced in M6).

- [ ] **Step 2: Write the sink receive function**

Create `src-tauri/src/audio/transport/udp/sink.rs`. It subscribes to the source, waits for STREAM_INFO, then receives audio into the jitter buffer; a separate consumer (the playback decode pump) reads from the jitter buffer. To keep the de-jitter and decode off the audio callback, the receive thread decodes and pushes f32 into the playback ring producer.

```rust
//! UDP sink I/O: subscribe to a source, receive audio, de-jitter, decode, and
//! feed the playback ring buffer.

use super::{jitter::{JitterBuffer, Pop}, packet};
use crate::audio::engine::decoder::decode_pcm_i16_le_to_f32;
use ringbuf::HeapProducer;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Result of the subscribe handshake.
pub struct Subscribed {
    pub socket: UdpSocket,
    pub info: packet::StreamInfo,
}

/// Sends SUBSCRIBE to `source_addr` and waits for STREAM_INFO. Returns the bound
/// socket (connected to the source) and the negotiated format.
pub fn subscribe(source_addr: &str, salt_b: u64, timeout: Duration) -> std::io::Result<Subscribed> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(source_addr)?;
    socket.set_read_timeout(Some(timeout))?;

    let mut sub = Vec::new();
    packet::encode_subscribe(salt_b, &mut sub);

    let mut buf = [0u8; 2048];
    let deadline = Instant::now() + timeout * 3;
    loop {
        socket.send(&sub)?;
        match socket.recv(&mut buf) {
            Ok(n) => {
                if let Some(info) = packet::decode_stream_info(&buf[..n]) {
                    socket.set_read_timeout(Some(Duration::from_millis(500)))?;
                    return Ok(Subscribed { socket, info });
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(e),
        }
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "no STREAM_INFO from source"));
        }
    }
}

/// Receives audio packets until `running` is cleared, decoding into `producer`.
/// Resends SUBSCRIBE every ~1s as a heartbeat.
pub fn receive_loop(
    socket: &UdpSocket,
    salt_b: u64,
    lost_after: usize,
    mut producer: HeapProducer<f32>,
    running: Arc<AtomicBool>,
) {
    let mut jb = JitterBuffer::new(lost_after);
    let mut buf = [0u8; 4096];
    let mut decoded: Vec<f32> = Vec::new();
    let mut sub = Vec::new();
    packet::encode_subscribe(salt_b, &mut sub);
    let mut last_hb = Instant::now();

    while running.load(Ordering::Relaxed) {
        if last_hb.elapsed() >= Duration::from_secs(1) {
            let _ = socket.send(&sub);
            last_hb = Instant::now();
        }
        match socket.recv(&mut buf) {
            Ok(n) => {
                if let Some((h, payload)) = packet::decode_audio(&buf[..n]) {
                    jb.push(h.seq, payload.to_vec());
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => break,
        }
        // Drain the jitter buffer into the playback ring.
        loop {
            match jb.pop() {
                Pop::Frame(pcm) => {
                    decode_pcm_i16_le_to_f32(&pcm, &mut decoded);
                    let _ = producer.push_slice(&decoded);
                }
                Pop::Gap => { /* silence handled by the output callback's underrun fill */ }
                Pop::Starved => break,
            }
        }
    }
}
```

> Note: `HeapProducer`/`HeapConsumer` come from `ringbuf` (already a dep). The playback ring is created by the orchestrator (Task 5.2) and split into producer (here) and consumer (the output callback).

- [ ] **Step 3: Verify compile**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8`
Expected: compiles (source.rs placeholder exists).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/transport/udp/mod.rs src-tauri/src/audio/transport/udp/sink.rs
git commit -m "feat(udp): sink subscribe + receive/de-jitter/decode loop"
```

### Task 5.2: `engine/sink.rs` — orchestrator

**Files:** Create `src-tauri/src/audio/engine/sink.rs`; Modify `src-tauri/src/audio/engine/mod.rs`

- [ ] **Step 1: Declare + write the orchestrator**

In `engine/mod.rs` add `pub mod sink;`. Create `src-tauri/src/audio/engine/sink.rs`:

```rust
//! Sink orchestrator: subscribe to a native source, open a cpal output stream
//! for the negotiated format, and pump received audio into it.

use super::playback::build_output_stream;
use super::super::stats::{emit_log, StreamStats};
use cpal::traits::{DeviceTrait, HostTrait};
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::AppHandle;

#[allow(clippy::too_many_arguments)]
pub fn run_sink(
    output_device_name: String,
    source_addr: String,
    latency_profile: String,
    app_handle: AppHandle,
) -> Result<(cpal::Stream, StreamStats), String> {
    // Subscribe (blocking, off the audio thread).
    let salt_b: u64 = Instant::now().elapsed().as_nanos() as u64 ^ 0x9E37_79B9_7F4A_7C15;
    let sub = super::super::transport::udp::sink::subscribe(
        &source_addr, salt_b, Duration::from_secs(2),
    )
    .map_err(|e| format!("subscribe failed: {e}"))?;
    let info = sub.info;
    emit_log(&app_handle, "success", format!(
        "Subscribed to {} ({}Hz, {}ch)", source_addr, info.sample_rate, info.channels
    ));

    // Find the chosen output device.
    let host = cpal::default_host();
    let device = host
        .output_devices()
        .map_err(|e| e.to_string())?
        .find(|d| d.name().map(|n| n == output_device_name).unwrap_or(false))
        .ok_or_else(|| format!("Output device not found: {output_device_name}"))?;

    let config = cpal::StreamConfig {
        channels: info.channels,
        sample_rate: cpal::SampleRate(info.sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    // Playback ring sized from the latency profile.
    let lp = super::latency::params(&latency_profile, false);
    let ring_samples = (info.sample_rate as usize) * (info.channels as usize)
        * (lp.adaptive_max_ms.max(lp.ring_ms) as usize) / 1000;
    let rb = HeapRb::<f32>::new(ring_samples.max(1024));
    let (prod, cons) = rb.split();

    let underruns = Arc::new(AtomicU64::new(0));
    let is_running = Arc::new(AtomicBool::new(true));

    // Receive thread.
    let socket = sub.socket;
    let running_net = is_running.clone();
    let lost_after = (lp.ring_ms / 5).max(2) as usize; // ~frames tolerance
    thread::spawn(move || {
        super::super::transport::udp::sink::receive_loop(
            &socket, salt_b, lost_after, prod, running_net,
        );
    });

    let stream = build_output_stream(&device, &config, cons, underruns.clone())
        .map_err(|e| e.to_string())?;

    Ok((
        stream,
        StreamStats {
            bytes_sent: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
            is_running,
            overruns: Arc::new(AtomicU64::new(0)),
            underruns,
        },
    ))
}
```

> The returned `StreamStats` reuses the existing struct so `manager` can stop the sink the same way it stops a source (`is_running` + dropping the stream). `bytes_sent`/`overruns` are unused on the sink side.

- [ ] **Step 2: Verify compile**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8`
Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/engine/mod.rs src-tauri/src/audio/engine/sink.rs
git commit -m "feat(engine): sink orchestrator (subscribe + output stream + pump)"
```

---

## Milestone 6 — Native UDP source

### Task 6.1: `transport/udp/source.rs` — serve a subscriber

**Files:** Create/replace `src-tauri/src/audio/transport/udp/source.rs`

- [ ] **Step 1: Write the source server**

Replace the `source.rs` placeholder with a function the source-side network thread calls. It binds a UDP socket, waits for a SUBSCRIBE, replies STREAM_INFO, then streams audio frames (PCM payloads produced by the existing capture→ring→encoder path) to the subscriber, numbering them with `seq`.

```rust
//! UDP source I/O: accept one subscriber and stream audio frames to it.

use super::packet::{self, AudioHeader, StreamInfo};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

pub struct UdpSource {
    socket: UdpSocket,
    peer: Option<SocketAddr>,
    seq: u64,
    start: Instant,
    info: StreamInfo,
    last_seen: Instant,
    out: Vec<u8>,
}

impl UdpSource {
    /// Binds the UDP source on `port`. Non-blocking so the network loop can
    /// interleave accept + send.
    pub fn bind(port: u16, sample_rate: u32, channels: u16) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", port))?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            peer: None,
            seq: 0,
            start: Instant::now(),
            info: StreamInfo { sample_rate, channels, salt_a: 0, flags: 0 },
            last_seen: Instant::now(),
            out: Vec::new(),
        })
    }

    /// Polls for a SUBSCRIBE/heartbeat. Should be called each loop iteration.
    pub fn poll_subscribe(&mut self) {
        let mut buf = [0u8; 512];
        while let Ok((n, addr)) = self.socket.recv_from(&mut buf) {
            if packet::decode_subscribe(&buf[..n]).is_some() {
                let mut info_out = Vec::new();
                packet::encode_stream_info(&self.info, &mut info_out);
                let _ = self.socket.send_to(&info_out, addr);
                self.peer = Some(addr);
                self.last_seen = Instant::now();
            }
        }
    }

    /// True if a subscriber is connected and recently seen.
    pub fn has_peer(&self) -> bool {
        self.peer.is_some() && self.last_seen.elapsed() < Duration::from_secs(5)
    }

    /// Sends one audio frame (raw PCM payload) to the subscriber.
    pub fn send_audio(&mut self, payload: &[u8]) {
        if !self.has_peer() {
            self.peer = None;
            return;
        }
        let peer = self.peer.expect("has_peer checked");
        let ts_us = self.start.elapsed().as_micros() as u64;
        let h = AudioHeader { flags: 0, seq: self.seq, ts_us };
        packet::encode_audio(&h, payload, &mut self.out);
        let _ = self.socket.send_to(&self.out, peer);
        self.seq = self.seq.wrapping_add(1);
    }
}
```

- [ ] **Step 2: Verify compile + all pure tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -6`
Expected: compiles; decoder/packet/jitter tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/transport/udp/source.rs
git commit -m "feat(udp): native source server (subscribe + stream audio frames)"
```

### Task 6.2: Wire the native source into `engine::run`

**Files:** Modify `src-tauri/src/audio/engine/mod.rs`

- [ ] **Step 1: Add a transport selector to the source path**

`engine::run` currently always does TCP. Add a `transport: String` param (`"tcp"` | `"udp"`, threaded from the command like in 2A). When `transport == "udp"`, the network thread uses a `UdpSource` instead of the TCP `current_stream`: each loop, call `src.poll_subscribe()`, and when `src.has_peer()` and prefilled, pop a chunk from `cons`, encode PCM (existing `encoder`), and `src.send_audio(&payload)` instead of writing to the TCP connection.

Concretely, inside the network-thread `while is_running` loop, gate the existing TCP connection-management + send blocks behind `if transport != "udp"`, and add a parallel UDP branch:

```rust
            if transport == "udp" {
                // Lazily bind the UDP source on first iteration.
                if udp_source.is_none() {
                    match super::transport::udp::source::UdpSource::bind(port, sample_rate_clone, device_channels_net) {
                        Ok(s) => { emit_log(&app_handle_net, "success", format!("Native UDP source on port {}", port)); udp_source = Some(s); }
                        Err(e) => { emit_log(&app_handle_net, "error", format!("UDP bind failed: {}", e)); thread::sleep(Duration::from_millis(500)); }
                    }
                }
                if let Some(src) = udp_source.as_mut() {
                    src.poll_subscribe();
                    if src.has_peer() {
                        let count = cons.pop_slice(&mut temp_buffer);
                        if count > 0 {
                            super::engine::encoder::encode_f32_to_pcm_i16_le(&temp_buffer[..count], &mut payload);
                            src.send_audio(&payload);
                            let _ = bytes_sent_clone.fetch_add(payload.len() as u64, Ordering::Relaxed);
                        } else {
                            thread::sleep(Duration::from_millis(1));
                        }
                    } else {
                        thread::sleep(Duration::from_millis(20));
                    }
                }
                // skip the TCP blocks this iteration
                continue;
            }
```

Declare `let mut udp_source: Option<super::transport::udp::source::UdpSource> = None;` near the other network-thread locals. Thread `transport` through `commands.rs` → `AudioCommand::Start` → `manager.rs` → `engine::run` (same pattern as 2A's `latency_profile`).

> The `continue` placement must be after the heartbeat/stats/quality emit blocks if you want stats while in UDP mode; simplest is to emit stats at the top of the loop. Keep the existing stats/quality blocks reachable by moving the `continue` to just before the TCP "Connection Management" block and letting UDP fall through to stats. The executor should verify stats still emit in UDP mode.

- [ ] **Step 2: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -6`
Expected: compiles; pure tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/engine/mod.rs src-tauri/src/audio/commands.rs src-tauri/src/audio/manager.rs
git commit -m "feat(engine): native UDP source transport option"
```

---

## Milestone 7 — Sink commands + manager wiring

### Task 7.1: `start_sink` / `stop_sink`

**Files:** Modify `commands.rs`, `manager.rs`, `lib.rs`

- [ ] **Step 1: Add a Sink command + AudioCommand variant**

- In `manager.rs`, add a variant `AudioCommand::StartSink { output_device: String, source_addr: String, latency_profile: String, app_handle: Box<AppHandle> }`. In the command loop, handle it: stop any current stream, then `match super::engine::sink::run_sink(output_device, source_addr, latency_profile, (*app_handle).clone())` storing the `(stream, stats)` in `current_stream_handle` and calling `stream.play()` (same as the source Start arm).
- In `commands.rs`, add:

```rust
#[tauri::command]
pub fn start_sink(
    state: State<'_, AudioState>,
    app_handle: AppHandle,
    output_device: String,
    source_addr: String,
    latency_profile: String,
) -> Result<(), AudioError> {
    state.tx.lock()
        .map_err(|_| AudioError::ChannelError("mutex poisoned".to_string()))?
        .send(AudioCommand::StartSink {
            output_device, source_addr, latency_profile, app_handle: Box::new(app_handle),
        })
        .map_err(|e| AudioError::ChannelError(e.to_string()))
}
```

`stop_sink` is unnecessary — reuse the existing `stop_stream` (it sends `AudioCommand::Stop`, which stops whatever stream is running, sink included).

- In `lib.rs`, register `audio::start_sink,` in `generate_handler!`.

- [ ] **Step 2: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -6`
Expected: compiles; all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/commands.rs src-tauri/src/audio/manager.rs src-tauri/src/lib.rs
git commit -m "feat(audio): start_sink command + manager wiring"
```

---

## Milestone 8 — Frontend: role + sink UI

### Task 8.1: Store state for the sink role

**Files:** Modify `src/stores/settings.ts`

- [ ] **Step 1: Add role + sink fields**

In `settings.ts`:
- Add `const role = ref("source");` (`"source"` | `"sink"`) and `const transport = ref("tcp");` (`"tcp"` | `"udp"`, source-side).
- Add `const outputDevice = ref("");` and `const sourceAddr = ref("");` (sink-side) and `const outputDevices = ref<string[]>([]);`.
- Persist them in `SettingsDict` (`role?`, `transport?`, `output_device?`, `source_addr?`) + load/save (same pattern as the other fields).
- Add an action `loadOutputDevices()` that calls `invoke("get_output_devices")` and stores the result (mirror `loadDevices`).
- Export the new refs + action.

- [ ] **Step 2: Verify**

Run: `pnpm typecheck && pnpm lint 2>&1 | tail -3`
Expected: green.

- [ ] **Step 3: Commit**

```bash
git add src/stores/settings.ts
git commit -m "feat(ui): sink role state (output device, source address)"
```

### Task 8.2: Sink start/stop in the stream store

**Files:** Modify `src/stores/stream.ts`

- [ ] **Step 1: Branch start on role**

In `stream.ts` `startStream`, at the top branch on `settings.role`:

```ts
    if (settings.role === "sink") {
      if (!settings.outputDevice) { statusText.value = "Select an output device"; return; }
      if (!settings.sourceAddr) { statusText.value = "Enter the source address"; return; }
      await settings.saveSettings();
      try {
        await invoke("start_sink", {
          outputDevice: settings.outputDevice,
          sourceAddr: settings.sourceAddr,
          latencyProfile: settings.latencyProfile,
        });
        isStreaming.value = true;
        statusText.value = `Playing from ${settings.sourceAddr}`;
      } catch (error) {
        statusText.value = "Error: " + error;
      }
      return;
    }
```

Also add `transport: settings.transport,` to the source-mode `invoke("start_stream", {...})` object (so the native UDP source option reaches the backend). `stopStream` already calls `invoke("stop_stream")`, which stops the sink too — no change.

- [ ] **Step 2: Verify**

Run: `pnpm typecheck && pnpm lint && pnpm test 2>&1 | tail -4`
Expected: green.

- [ ] **Step 3: Commit**

```bash
git add src/stores/stream.ts
git commit -m "feat(ui): start sink playback from the stream store"
```

### Task 8.3: Connection tab — role, output device, source address

**Files:** Modify `src/components/tabs/ConnectionTab.vue`

- [ ] **Step 1: Add a role selector and sink fields**

At the top of `ConnectionTab.vue`'s template, add a role `SelectField` (`settings.role`: Source / Sink). When `settings.role === "sink"`, show an output-device `SelectField` (bound to `settings.outputDevice`, options from `settings.outputDevices`) and a source-address `InputField` (`settings.sourceAddr`, placeholder `192.168.1.50:4953`), and hide the source-only sections. When `settings.role === "source"`, additionally show a transport `SelectField` (`settings.transport`: TCP / Native UDP). On mount (in `<script setup>`), call `settings.loadOutputDevices()`.

```vue
      <SelectField id="role-select" v-model="settings.role" label="Role" :disabled="stream.isStreaming">
        <option value="source">Source (capture & send)</option>
        <option value="sink">Sink (receive & play)</option>
      </SelectField>
```

(Sink fields, shown with `v-if="settings.role === 'sink'"`:)
```vue
      <SelectField id="output-device" v-model="settings.outputDevice" label="Output Device" :disabled="stream.isStreaming">
        <option v-for="d in settings.outputDevices" :key="d" :value="d">{{ d }}</option>
      </SelectField>
      <InputField id="source-addr" v-model="settings.sourceAddr" label="Source Address" placeholder="192.168.1.50:4953" :disabled="stream.isStreaming" />
```

- [ ] **Step 2: Verify**

Run: `pnpm typecheck && pnpm lint && pnpm test && pnpm format:check 2>&1 | tail -4`
Expected: green (run `pnpm format` if prettier flags the new markup).

- [ ] **Step 3: Commit**

```bash
git add src/components/tabs/ConnectionTab.vue
git commit -m "feat(ui): role selector + sink output/source fields"
```

---

## Milestone 9 — Verification

### Task 9.1: Loopback integration test (source ↔ sink)

**Files:** Create `src-tauri/src/audio/transport/udp/tests_loopback.rs`; Modify `transport/udp/mod.rs`

- [ ] **Step 1: Write a localhost source↔sink test**

In `transport/udp/mod.rs` add `#[cfg(test)] mod tests_loopback;`. Create `tests_loopback.rs`:

```rust
//! End-to-end UDP test over localhost: a UdpSource and a subscribing socket
//! exchange the handshake and one audio frame.

use super::{packet, source::UdpSource};
use std::net::UdpSocket;
use std::time::Duration;

#[test]
fn subscribe_then_receive_one_audio_frame() {
    let mut src = UdpSource::bind(0, 48000, 2).expect("bind");
    // Discover the OS-assigned port via a second bind trick: rebind is not
    // possible, so expose the local addr.
    let src_addr = src_local_addr(&src);

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

fn src_local_addr(src: &UdpSource) -> String {
    src.local_addr()
}
```

> Add a `pub fn local_addr(&self) -> String { self.socket.local_addr().map(|a| a.to_string()).unwrap_or_default() }` method to `UdpSource` in `source.rs` to support the test.

- [ ] **Step 2: Run it**

Run: `cargo test --manifest-path src-tauri/Cargo.toml udp 2>&1 | tail -10`
Expected: the loopback test + all pure udp tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/transport/udp/mod.rs src-tauri/src/audio/transport/udp/tests_loopback.rs src-tauri/src/audio/transport/udp/source.rs
git commit -m "test(udp): localhost source<->sink handshake + audio frame"
```

### Task 9.2: Full gate + manual end-to-end

- [ ] **Step 1: Complete check suite**

Run:
```bash
pnpm test && pnpm typecheck && pnpm lint && pnpm format:check && \
cargo test --manifest-path src-tauri/Cargo.toml && \
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```
Expected: all green; Rust tests up by ~16 (decoder 3, packet 3, jitter 5, loopback 1, + existing).

- [ ] **Step 2: Manual end-to-end (two instances)**

On machine/instance A: Role = Source, Transport = Native UDP, pick an input device, Start.
On machine/instance B: Role = Sink, pick an Output Device, Source Address = `A_IP:port`, Start.
Verify: audio captured on A plays out B's selected output device; killing/adding network jitter causes occasional micro-glitches, never a hard stall; stopping either side ends cleanly.

- [ ] **Step 3: Final commit (if needed)**

```bash
git add -A && git commit -m "chore: Phase 2B-i verification fixes"
```

---

## Self-Review (author)

- **Spec coverage (2B-i):** Sink/playback (§5.1) → M4+M5.2; decoder (§5.2) → M1; UDP packet/jitter (§5.3) → M2+M3; handshake SUBSCRIBE/STREAM_INFO (§5.5, plaintext parts) → M2+M5+M6; roles/commands/UI → M6.2+M7+M8. **Deferred:** crypto (§5.4) → Plan 2B-ii; mDNS (§5.6) → Plan 2B-iii; drift handling (§5.7) → folded into 2B-ii/iii or a follow-up (jitter currently fixed-depth).
- **Type consistency:** `decode_pcm_i16_le_to_f32(&[u8],&mut Vec<f32>)`; packet `encode_/decode_{audio,subscribe,stream_info}` with `AudioHeader{flags,seq,ts_us}` / `StreamInfo{sample_rate,channels,salt_a,flags}`; `JitterBuffer::{new(usize),push(u64,Vec<u8>),pop()->Pop}`; `build_output_stream(&Device,&StreamConfig,HeapConsumer<f32>,Arc<AtomicU64>)`; `run_sink(String,String,String,AppHandle)`. New params `transport` (source) and the sink command args are threaded command→manager→engine in the established order.
- **No placeholders:** every code step is complete. The one structural caveat (M6.2 `continue` placement vs stats emission) is called out explicitly with what to verify, not left vague.

## Next plans

- **2B-ii — Encryption (AEAD/PSK):** `transport/udp/crypto.rs` (HKDF + ChaCha20-Poly1305 + replay window, pure+tests); set the `flags` encrypted bit; PSK UI field; seal on source, open on sink. Adds `chacha20poly1305`, `hkdf`, `sha2`.
- **2B-iii — Discovery (mDNS):** `transport/discovery.rs` (advertise on source, browse on sink) + `list_sources` command + UI picker. Adds `mdns-sd`. Also fold in the clock-drift handling (§5.7) as a tested `drift` helper in the jitter path.
