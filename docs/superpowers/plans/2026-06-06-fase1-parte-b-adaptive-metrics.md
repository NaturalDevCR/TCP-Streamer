# Phase 1 — Part B: Real Adaptive Buffer + Honest Metrics — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the cosmetic "Adaptive Buffer" with a real target-occupancy controller that resizes the latency target in response to real glitches, and replace the misleading `write()`-timing "jitter/latency" with honest underrun/overrun counters plus best-effort TCP RTT from `TCP_INFO`.

**Architecture:** A pure `engine::buffer::AdaptiveBuffer` controller (testable, no I/O) drives the consumer drain loop's latency target; the ring buffer is allocated once at `max_buffer_ms` so the target never needs reallocation. A new `metrics` module owns underrun/overrun counters, a pure quality-scoring function, and per-platform RTT via `getsockopt(TCP_INFO)` (`libc` on Unix; `None`/"n/a" elsewhere). The `QualityEvent` payload is reshaped to expose honest fields, with matching frontend types/store/labels.

**Tech Stack:** Rust (Tauri 2), `cpal 0.15.3`, `ringbuf 0.3.3`, `libc 0.2` (Unix only); Vue 3 + Pinia + Vitest.

**Spec:** `docs/superpowers/specs/2026-06-05-fase1-estabilizacion-design.md` §5.2, §5.4.
**Depends on:** Part A complete (engine modules exist; `StreamStats.overruns` exists). **Line numbers in `stream.rs` shifted during Part A — anchor edits by the quoted code/comment, not line number.**

---

## File Structure

| File | Responsibility | Status |
|---|---|---|
| `src-tauri/src/audio/engine/buffer.rs` | **(pure)** target-occupancy adaptive controller | Create |
| `src-tauri/src/audio/metrics.rs` | underrun/overrun counters, **(pure)** quality score, per-OS RTT | Create |
| `src-tauri/src/audio/mod.rs` | `pub mod metrics;` | Modify |
| `src-tauri/src/audio/engine/mod.rs` | `pub mod buffer;` | Modify |
| `src-tauri/Cargo.toml` | add `libc` under `[target.'cfg(unix)'.dependencies]` | Modify |
| `src-tauri/src/audio/stats.rs` | add `underruns` counter; reshape `QualityEvent` | Modify |
| `src-tauri/src/audio/stream.rs` | size ring at max; wire controller + counters; emit honest quality | Modify |
| `src/types/events.ts` | reshape `QualityEvent` | Modify |
| `src/stores/stream.ts` | listen with new fields; drop jitter/avgLatency/errorCount | Modify |
| `src/components/StatsBar.vue` | show RTT / Underruns / Dropped | Modify |
| `src/stores/__tests__/stream.test.ts` | update for new quality fields | Modify |

---

## Milestone 1 — `engine/buffer.rs`: adaptive controller (TDD)

### Task 1.1: Pure target-occupancy controller

**Files:**
- Create: `src-tauri/src/audio/engine/buffer.rs`
- Modify: `src-tauri/src/audio/engine/mod.rs`

- [ ] **Step 1: Declare the module**

In `src-tauri/src/audio/engine/mod.rs` add:
```rust
pub mod buffer;
```

- [ ] **Step 2: Write the controller + tests**

Create `src-tauri/src/audio/engine/buffer.rs`:

```rust
//! Adaptive latency-target controller (target-occupancy model).
//!
//! The ring buffer is allocated once at the maximum size; this controller only
//! decides how many milliseconds of audio we aim to keep buffered ("target"),
//! growing it when glitches occur (more jitter tolerance, more latency) and
//! shrinking it toward the minimum when the stream has been stable (less
//! latency). It performs no I/O so it is fully unit-testable.

/// Adaptive controller for the buffered-latency target, in milliseconds.
#[derive(Debug, Clone)]
pub struct AdaptiveBuffer {
    min_ms: u32,
    max_ms: u32,
    step_ms: u32,
    target_ms: u32,
    stable_ticks: u32,
    stable_threshold: u32,
}

impl AdaptiveBuffer {
    /// `stable_threshold` = consecutive glitch-free ticks required before the
    /// target is lowered one step. `initial_ms` is clamped into `[min, max]`.
    pub fn new(min_ms: u32, max_ms: u32, step_ms: u32, initial_ms: u32, stable_threshold: u32) -> Self {
        let max_ms = max_ms.max(min_ms);
        Self {
            min_ms,
            max_ms,
            step_ms: step_ms.max(1),
            target_ms: initial_ms.clamp(min_ms, max_ms),
            stable_ticks: 0,
            stable_threshold: stable_threshold.max(1),
        }
    }

    pub fn target_ms(&self) -> u32 {
        self.target_ms
    }

    /// Advances one control tick. `had_glitch` = an underrun or overrun was
    /// observed since the previous tick. Returns `Some(new_target)` only when
    /// the target actually changed.
    pub fn on_tick(&mut self, had_glitch: bool) -> Option<u32> {
        let prev = self.target_ms;
        if had_glitch {
            self.stable_ticks = 0;
            self.target_ms = (self.target_ms + self.step_ms).min(self.max_ms);
        } else {
            self.stable_ticks += 1;
            if self.stable_ticks >= self.stable_threshold {
                self.stable_ticks = 0;
                self.target_ms = self.target_ms.saturating_sub(self.step_ms).max(self.min_ms);
            }
        }
        (self.target_ms != prev).then_some(self.target_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_target_is_clamped() {
        assert_eq!(AdaptiveBuffer::new(2000, 8000, 500, 500, 3).target_ms(), 2000);
        assert_eq!(AdaptiveBuffer::new(2000, 8000, 500, 99000, 3).target_ms(), 8000);
        assert_eq!(AdaptiveBuffer::new(2000, 8000, 500, 4000, 3).target_ms(), 4000);
    }

    #[test]
    fn glitch_raises_target_up_to_max() {
        let mut a = AdaptiveBuffer::new(2000, 3000, 500, 2000, 3);
        assert_eq!(a.on_tick(true), Some(2500));
        assert_eq!(a.on_tick(true), Some(3000));
        assert_eq!(a.on_tick(true), None); // already at max, no change
        assert_eq!(a.target_ms(), 3000);
    }

    #[test]
    fn stability_lowers_target_after_threshold() {
        let mut a = AdaptiveBuffer::new(2000, 8000, 500, 3000, 3);
        assert_eq!(a.on_tick(false), None); // 1
        assert_eq!(a.on_tick(false), None); // 2
        assert_eq!(a.on_tick(false), Some(2500)); // 3rd glitch-free tick lowers
        assert_eq!(a.on_tick(false), None); // counter reset
        assert_eq!(a.on_tick(false), None);
        assert_eq!(a.on_tick(false), Some(2000)); // lowers again
        assert_eq!(a.on_tick(false), None); // already at min after threshold
        assert_eq!(a.on_tick(false), None);
        assert_eq!(a.on_tick(false), None); // floored at min, no change
    }

    #[test]
    fn glitch_resets_stability_counter() {
        let mut a = AdaptiveBuffer::new(2000, 8000, 500, 3000, 3);
        a.on_tick(false);
        a.on_tick(false);
        a.on_tick(true); // raises to 3500, resets counter
        assert_eq!(a.target_ms(), 3500);
        assert_eq!(a.on_tick(false), None); // counter restarts from 0
        assert_eq!(a.on_tick(false), None);
        assert_eq!(a.on_tick(false), Some(3000));
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml engine::buffer 2>&1 | tail -10`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/engine/mod.rs src-tauri/src/audio/engine/buffer.rs
git commit -m "feat(engine): add tested adaptive latency-target controller"
```

---

## Milestone 2 — `metrics.rs`: counters, scoring, RTT

### Task 2.1: Add the `libc` dependency (Unix)

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add a target-specific dependency**

In `src-tauri/Cargo.toml`, after the `[dependencies]` block, add:

```toml
[target.'cfg(unix)'.dependencies]
libc = "0.2"
```

- [ ] **Step 2: Verify it resolves**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: builds (libc was already a transitive dep, so no new download).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "build: add libc dependency for Unix TCP_INFO RTT"
```

### Task 2.2: Pure quality-scoring (TDD)

**Files:**
- Create: `src-tauri/src/audio/metrics.rs`
- Modify: `src-tauri/src/audio/mod.rs`

- [ ] **Step 1: Declare the module**

In `src-tauri/src/audio/mod.rs`, add `pub mod metrics;` to the module list (alphabetically, after `pub mod manager;`).

- [ ] **Step 2: Write the scoring function + tests**

Create `src-tauri/src/audio/metrics.rs`:

```rust
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
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml metrics::tests:: 2>&1 | tail -10`
Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/mod.rs src-tauri/src/audio/metrics.rs
git commit -m "feat(metrics): add tested honest quality scoring"
```

### Task 2.3: Per-platform TCP RTT via getsockopt(TCP_INFO)

**Files:**
- Modify: `src-tauri/src/audio/metrics.rs`

- [ ] **Step 1: Append the platform-gated RTT readers**

Append to `src-tauri/src/audio/metrics.rs`. Field/constant names verified against `libc 0.2`: Linux `tcp_info.tcpi_rtt`/`tcpi_rttvar` are microseconds; macOS `tcp_connection_info.tcpi_srtt`/`tcpi_rttvar` are milliseconds. Windows and other targets return `None` ("n/a") until validated on that hardware — this is the spec's documented best-effort behavior, not a stub to fill in.

```rust
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
```

- [ ] **Step 2: Add an integration test over a loopback socket pair (Unix dev platform)**

Append this test module to `metrics.rs`. It opens a real localhost TCP connection so `tcp_rtt` exercises the actual syscall:

```rust
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
        assert!(s.srtt_ms >= 0.0 && s.srtt_ms < 1000.0, "loopback srtt sane: {}", s.srtt_ms);
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml metrics:: 2>&1 | tail -12`
Expected: scoring tests + the loopback RTT test pass on macOS/Linux.

> If the loopback test returns `None` on the CI platform, relax it to `assert!(sample.is_some() || cfg!(not(any(target_os="linux",target_os="macos"))))` — but on macOS/Linux dev hardware it should be `Some`.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/metrics.rs
git commit -m "feat(metrics): best-effort TCP RTT via getsockopt(TCP_INFO)"
```

---

## Milestone 3 — Reshape the quality event + counters

### Task 3.1: Add `underruns` to `StreamStats` and reshape `QualityEvent`

**Files:**
- Modify: `src-tauri/src/audio/stats.rs`

- [ ] **Step 1: Add the underruns counter**

In `StreamStats` (which Part A gave an `overruns` field), add:

```rust
    /// Times the consumer was starved (buffer below one chunk after prefill).
    pub underruns: Arc<AtomicU64>,
```

- [ ] **Step 2: Reshape `QualityEvent`**

Replace the current `QualityEvent` struct (fields `score, jitter, avg_latency, buffer_health, error_count`) with the honest shape:

```rust
#[derive(Clone, Serialize)]
pub struct QualityEvent {
    pub score: u8,
    /// Best-effort smoothed RTT in ms; `None` => "n/a".
    pub rtt_ms: Option<f32>,
    /// Best-effort RTT variance in ms; `None` => "n/a".
    pub rtt_var_ms: Option<f32>,
    pub underruns: u64,
    pub dropped: u64,
    pub buffer_health: f32,
}
```

- [ ] **Step 3: Verify (expect errors in `stream.rs` until Task 3.2)**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8`
Expected: FAIL — `stream.rs` still constructs the old `QualityEvent` and the old `StreamStats`. Fixed in Task 3.2. Do not commit yet.

### Task 3.2: Wire the controller, counters, and honest event into the consumer loop

**Files:**
- Modify: `src-tauri/src/audio/stream.rs`

> All edits below are in the network-thread closure of `start_audio_stream`. Anchor by the quoted code; Part A shifted line numbers.

- [ ] **Step 1: Allocate the ring buffer at the maximum size**

Find the ring-buffer sizing (the `let ring_buffer_size = (sample_rate as usize) * (device_channels as usize) * (adjusted_ring_buffer_duration_ms as usize) / 1000;`). Change it to size for `max_buffer_ms` so the target never needs reallocation:

```rust
    let ring_capacity_ms = max_buffer_ms.max(adjusted_ring_buffer_duration_ms);
    let ring_buffer_size =
        (sample_rate as usize) * (device_channels as usize) * (ring_capacity_ms as usize) / 1000;
```

- [ ] **Step 2: Create the underruns counter alongside overruns**

Where Part A created `let overruns = Arc::new(AtomicU64::new(0));`, add:

```rust
    let underruns = Arc::new(AtomicU64::new(0));
```

Clone both for the network thread (near the other `*_clone` bindings):

```rust
    let overruns_net = overruns.clone();
    let underruns_net = underruns.clone();
```

- [ ] **Step 3: Instantiate the adaptive controller in the network thread**

Inside the network-thread closure, replace the cosmetic `let mut current_buffer_ms = adj_ms_u32;` line with the real controller (one tick per quality interval; lower after ~6 stable ticks ≈ 30 s at a 5 s interval):

```rust
        let mut adaptive = super::engine::buffer::AdaptiveBuffer::new(
            adaptive_min_ms,
            max_buffer_ms.max(adaptive_min_ms),
            super::constants::ADAPTIVE_BUFFER_STEP_MS,
            adj_ms_u32,
            6,
        );
        let mut glitch_baseline: u64 = 0;
```

- [ ] **Step 4: Count an underrun at the starvation point**

Find the starvation branch (`} else if current_stream.is_some() && current_buffered < min_chunk_samples && prefilled {` … `thread::sleep(Duration::from_millis(2)); continue;`). Increment the underrun counter there:

```rust
            } else if current_stream.is_some() && current_buffered < min_chunk_samples && prefilled {
                underruns_net.fetch_add(1, Ordering::Relaxed);
                thread::sleep(Duration::from_millis(2));
                continue;
```

- [ ] **Step 5: Derive prefill from the current target**

Find `let prefill_samples = sample_rate as usize * device_channels_net as usize / PREFILL_FRACTION;`. Replace with a value derived from the adaptive target so a larger target buffers more before transmitting:

```rust
        let prefill_ms = adaptive.target_ms();
        let prefill_samples =
            sample_rate as usize * device_channels_net as usize * prefill_ms as usize / 1000;
```

> `prefill_samples` is recomputed on reconnect in Step 6's block when the target changed; for the initial fill this value is sufficient.

- [ ] **Step 6: Replace the cosmetic adaptive block with the real controller + honest quality event**

Find the quality-emit block (the `if last_quality_emit.elapsed() >= Duration::from_secs(QUALITY_REPORT_INTERVAL_SECS) {` … through the cosmetic `Adaptive Buffer` block that ends near the `buffer-resize` emit). Replace the whole block with:

```rust
            if last_quality_emit.elapsed() >= Duration::from_secs(QUALITY_REPORT_INTERVAL_SECS) {
                let occupied = cons.len();
                let capacity = cons.capacity();
                let buffer_health = 1.0 - (occupied as f32 / capacity as f32);
                let occupancy_ratio = occupied as f32 / capacity as f32;

                let total_glitches =
                    underruns_net.load(Ordering::Relaxed) + overruns_net.load(Ordering::Relaxed);
                let glitches_delta = total_glitches.saturating_sub(glitch_baseline);
                glitch_baseline = total_glitches;

                // Best-effort RTT from the live socket, if connected.
                let rtt = current_stream.as_ref().and_then(|s| {
                    let super::stream::StreamSocket::Tcp(tcp) = s;
                    super::metrics::tcp_rtt(tcp)
                });

                let score = super::metrics::quality_score(
                    glitches_delta,
                    occupancy_ratio,
                    rtt.map(|r| r.srtt_ms),
                );

                let _ = app_handle_net.emit(
                    "quality-event",
                    QualityEvent {
                        score,
                        rtt_ms: rtt.map(|r| r.srtt_ms),
                        rtt_var_ms: rtt.map(|r| r.rttvar_ms),
                        underruns: underruns_net.load(Ordering::Relaxed),
                        dropped: overruns_net.load(Ordering::Relaxed),
                        buffer_health,
                    },
                );
                last_quality_emit = Instant::now();

                // Real adaptive control: one tick per quality interval.
                if enable_adaptive_buffer {
                    if let Some(new_target) = adaptive.on_tick(glitches_delta > 0) {
                        emit_log(
                            &app_handle_net,
                            "info",
                            format!("Adaptive Buffer: target now {}ms", new_target),
                        );
                        let _ = app_handle_net.emit(
                            "buffer-resize",
                            BufferResizeEvent {
                                new_size_ms: new_target,
                                reason: if glitches_delta > 0 { "Glitches detected" } else { "Stable" }
                                    .to_string(),
                            },
                        );
                    }
                }
            }
```

> Remove the now-unused `current_buffer_ms`, `last_buffer_check`, and the old `error_count`/`latency_samples`/`jitter` machinery if they are no longer referenced. Run the compiler to find dead bindings.

- [ ] **Step 7: Add `underruns` to the returned `StreamStats`**

In the `Ok((audio_stream, StreamStats { ... }))` at the end of `start_audio_stream`, add `underruns,` next to `overruns,`.

- [ ] **Step 8: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -10`
Expected: compiles clean (clippy-clean, no unused warnings); all tests pass.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/audio/stats.rs src-tauri/src/audio/stream.rs
git commit -m "feat(audio): real adaptive target + honest underrun/overrun/RTT quality event"
```

---

## Milestone 4 — Frontend: honest metrics surface

### Task 4.1: Reshape the TS event type

**Files:**
- Modify: `src/types/events.ts`

- [ ] **Step 1: Replace `QualityEvent`**

In `src/types/events.ts`, replace the `QualityEvent` interface with:

```ts
/** Streaming quality metrics (honest signals) */
export interface QualityEvent {
  score: number;
  rtt_ms: number | null;
  rtt_var_ms: number | null;
  underruns: number;
  dropped: number;
  buffer_health: number;
}
```

- [ ] **Step 2: Typecheck**

Run: `npm run typecheck 2>&1 | tail -15`
Expected: FAIL in `src/stores/stream.ts` (old fields). Fixed in Task 4.2.

### Task 4.2: Update the stream store

**Files:**
- Modify: `src/stores/stream.ts`

- [ ] **Step 1: Replace the quality state + listener**

In `src/stores/stream.ts`, replace the quality refs (`jitter`, `avgLatency`, `bufferHealth`, `errorCount`) with the honest ones:

```ts
  // Quality
  const qualityScore = ref(0);
  const rttMs = ref<number | null>(null);
  const rttVarMs = ref<number | null>(null);
  const underruns = ref(0);
  const dropped = ref(0);
  const bufferHealth = ref(0);
  const bufferMs = ref(0);
```

Replace the `quality-event` listener body with:

```ts
    await listen("quality-event", (event: { payload: QualityEvent }) => {
      const q = event.payload;
      qualityScore.value = q.score;
      rttMs.value = q.rtt_ms;
      rttVarMs.value = q.rtt_var_ms;
      underruns.value = q.underruns;
      dropped.value = q.dropped;
      bufferHealth.value = q.buffer_health;

      if (q.score < 50 && !qualityWarningShown) {
        addToast(`Network quality degraded to ${q.score}`, "warning");
        qualityWarningShown = true;
      } else if (q.score >= 70 && qualityWarningShown) {
        addToast(`Network quality recovered to ${q.score}`, "success");
        qualityWarningShown = false;
      }
    });
```

Add `import type { QualityEvent } from "../types/events";` to the existing type import line, and update the store's `return { ... }` to export `rttMs, rttVarMs, underruns, dropped` and drop `jitter, avgLatency, errorCount`.

- [ ] **Step 2: Typecheck**

Run: `npm run typecheck 2>&1 | tail -15`
Expected: FAIL only in `src/components/StatsBar.vue` (uses `jitter`). Fixed in Task 4.3.

### Task 4.3: Update the stats bar labels

**Files:**
- Modify: `src/components/StatsBar.vue`

- [ ] **Step 1: Replace the Jitter/Buffer stats with honest ones**

In `src/components/StatsBar.vue`, replace the `stats` computed entries that referenced `jitter` with RTT/Underruns/Dropped:

```ts
const stats = computed(() => [
  { label: "Uptime", value: stream.formattedUptime, style: undefined },
  { label: "Transferred", value: stream.formattedBytes, style: undefined },
  { label: "Bitrate", value: stream.bitrateKbps.toFixed(1) + " kbps", style: undefined },
  {
    label: "Quality",
    value: `${stream.qualityLabel.text} (${stream.qualityScore})`,
    dot: true,
    dotColor: stream.qualityLabel.color,
    style: undefined,
  },
  { label: "RTT", value: stream.rttMs === null ? "n/a" : stream.rttMs.toFixed(1) + " ms", style: undefined },
  { label: "Underruns", value: String(stream.underruns), style: undefined },
]);
```

- [ ] **Step 2: Run all frontend checks**

Run: `npm run typecheck && npm test && npm run lint`
Expected: typecheck clean; tests need the update in Task 4.4; lint clean.

### Task 4.4: Update store tests

**Files:**
- Modify: `src/stores/__tests__/stream.test.ts`

- [ ] **Step 1: Update any assertions referencing removed fields**

Open `src/stores/__tests__/stream.test.ts`. Replace assertions on `jitter`/`avgLatency`/`errorCount` with `rttMs`/`underruns`/`dropped`. If a test simulates a `quality-event`, update its payload to the new shape:

```ts
const payload = { score: 95, rtt_ms: 4.2, rtt_var_ms: 1.1, underruns: 0, dropped: 0, buffer_health: 0.9 };
```

- [ ] **Step 2: Run the tests**

Run: `npm test 2>&1 | tail -8`
Expected: all pass.

- [ ] **Step 3: Commit**

```bash
git add src/types/events.ts src/stores/stream.ts src/components/StatsBar.vue src/stores/__tests__/stream.test.ts
git commit -m "feat(ui): surface honest RTT/underruns/dropped metrics"
```

---

## Milestone 5 — Verification

### Task 5.1: Full green gate + behavioural check

- [ ] **Step 1: Complete check suite**

Run:
```bash
npm test && npm run typecheck && npm run lint && \
cargo test --manifest-path src-tauri/Cargo.toml && \
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```
Expected: all green; Rust test count up by ~14 (controller 4, scoring 5, rtt 1, plus Part A).

- [ ] **Step 2: Manual behavioural verification**

Run: `npm run tauri dev`, then with Adaptive Buffer enabled stream to Snapcast and:
1. Confirm the Stats bar shows a real **RTT** (or "n/a" on an unsupported platform) and **Underruns** that stay at 0 on a healthy LAN.
2. Induce jitter (e.g. `sudo tc qdisc add dev <iface> root netem delay 80ms 40ms` on Linux, or stream over congested WiFi). Confirm: underruns appear, the log shows "Adaptive Buffer: target now …ms" rising, and after the network calms the target steps back down.
3. Confirm `dropped` increments only under real overrun (producer outpacing a stalled socket), not during normal operation.

Expected: the adaptive target visibly tracks real conditions; metrics reflect reality, not `write()` timing.

- [ ] **Step 3: Final commit (if tweaks needed)**

```bash
git add -A && git commit -m "chore: Phase 1 Part B verification fixes"
```

---

## Self-Review (author)

- **Spec coverage:** §5.2 adaptive (real target-occupancy) → M1 + M3.2; §5.4 honest metrics (underrun/overrun + RTT + scoring) → M2 + M3; frontend surface (§5.9 metrics rename) → M4.
- **Type consistency:** `AdaptiveBuffer::{new(min,max,step,initial,stable_threshold), target_ms(), on_tick(bool)->Option<u32>}`; `quality_score(u64, f32, Option<f32>)->u8`; `tcp_rtt(&TcpStream)->Option<RttSample>`; `RttSample{srtt_ms,rttvar_ms}`; `QualityEvent{score,rtt_ms:Option,rtt_var_ms:Option,underruns,dropped,buffer_health}` matches the TS `{score,rtt_ms:number|null,rtt_var_ms,underruns,dropped,buffer_health}`. `StreamStats` gains `underruns` (this part) next to `overruns` (Part A).
- **No placeholders:** the non-Unix `tcp_rtt` returning `None` is the spec's documented best-effort behavior, not an unfinished stub.
- **Note for executor:** `StreamSocket` still lives in `stream.rs` in Part B (Part C dissolves it); the RTT read pattern-matches `super::stream::StreamSocket::Tcp(tcp)`.
