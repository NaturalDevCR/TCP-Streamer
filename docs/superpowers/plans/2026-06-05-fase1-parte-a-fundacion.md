# Phase 1 — Part A: Foundation & Critical Fixes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Commit the verified TS migration baseline, fix the broken DSCP/QoS mapping end-to-end, make the audio callback real-time safe, make the "Buffer Size" control actually affect cpal, and lay the testable engine-decomposition foundation — all while keeping the build green.

**Architecture:** Introduce focused, unit-testable modules under `src-tauri/src/audio/{engine,transport}/` and have the existing `stream.rs` call into them, rather than rewriting it wholesale. Pure logic (DSCP mapping, PCM encoding, config ranking, buffer-size negotiation) gets extracted with tests; the cpal callback is rebuilt to move the SPSC producer in (no `Arc<Mutex>`, no per-callback allocation).

**Tech Stack:** Rust (Tauri 2), `cpal 0.15.3`, `ringbuf 0.3.3`, `socket2 0.5`; Vue 3 + Pinia + Vitest on the frontend.

**Spec:** `docs/superpowers/specs/2026-06-05-fase1-estabilizacion-design.md`

**Scope of this plan (Part A):** Spec §6 (Fase 0), §5.5 (DSCP), §5.1 (RT-safe capture), §5.3 (effective Buffer Size), the device/encoder/capture pieces of §4, and the §5.8 tests for them.
**Deferred to Part B:** §5.2 (real adaptive buffer) + §5.4 (honest metrics).
**Deferred to Part C:** §5.6 (reconnection consolidation) + §5.7 (non-blocking HTTP handshake) + full `transport` trait.

---

## File Structure

| File                                    | Responsibility                                                          | Status          |
| --------------------------------------- | ----------------------------------------------------------------------- | --------------- |
| `src-tauri/src/audio/transport/mod.rs`  | Declares the transport submodules                                       | Create          |
| `src-tauri/src/audio/transport/dscp.rs` | **(pure)** DSCP strategy → IP TOS byte                                  | Create          |
| `src-tauri/src/audio/engine/mod.rs`     | Declares the engine submodules                                          | Create          |
| `src-tauri/src/audio/engine/device.rs`  | **(pure)** config ranking/selection over candidate descriptors          | Create          |
| `src-tauri/src/audio/engine/encoder.rs` | **(pure)** f32 → PCM i16 LE encoding                                    | Create          |
| `src-tauri/src/audio/engine/capture.rs` | RT-safe cpal stream builder + buffer-size negotiation                   | Create          |
| `src-tauri/src/audio/mod.rs`            | Wire new modules (`pub mod engine; pub mod transport;`)                 | Modify          |
| `src-tauri/src/audio/stream.rs`         | Call into new modules; remove `Arc<Mutex>`; apply DSCP in client+server | Modify          |
| `src-tauri/src/audio/stats.rs`          | Add `overruns` counter to `StreamStats`                                 | Modify          |
| `src/components/tabs/AdvancedTab.vue`   | Confirm DSCP option values match backend keys                           | Modify (verify) |
| `src/stores/__tests__/settings.test.ts` | Lock the DSCP key contract                                              | Modify          |
| `.gitignore`                            | Ignore build artifact + `.DS_Store`                                     | Modify          |

---

## Milestone 0 — Baseline & repo hygiene

### Task 0.1: Untrack build artifacts and update .gitignore

**Files:**

- Modify: `.gitignore`

- [ ] **Step 1: Inspect whether the 10 MB binary and .DS_Store are tracked**

Run: `git ls-files | grep -E '(^|/)tcp-streamer$|\.DS_Store$' || echo "none tracked"`
Expected: lists `tcp-streamer` and/or `.DS_Store` if tracked, else `none tracked`.

- [ ] **Step 2: Append ignore rules**

Add these lines to `.gitignore` (append; do not remove existing entries):

```gitignore
# Build artifacts / OS cruft
/tcp-streamer
.DS_Store
**/.DS_Store
```

- [ ] **Step 3: Untrack any artifacts that were tracked**

Run (each is a no-op if the file was not tracked):

```bash
git rm --cached --ignore-unmatch tcp-streamer
git rm --cached --ignore-unmatch .DS_Store
git ls-files | grep -E '(^|/)tcp-streamer$|\.DS_Store$' || echo "clean"
```

Expected: final line prints `clean`.

- [ ] **Step 4: Commit**

```bash
git add .gitignore
git commit -m "chore: stop tracking build artifact and .DS_Store"
```

### Task 0.2: Commit the verified TS migration as baseline

**Files:** (working-tree migration already present and verified green)

- [ ] **Step 1: Re-verify the baseline is green**

Run:

```bash
npm test && npm run typecheck && npm run lint && cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: Vitest 17 passed; vue-tsc no output; eslint no output; `cargo test` `test result: ok. 10 passed`.

- [ ] **Step 2: Stage everything except ignored artifacts**

Run: `git add -A && git status --short | head -40`
Expected: the JS→TS migration, tooling configs, and Rust module split are staged; the binary/`.DS_Store` are NOT listed.

- [ ] **Step 3: Commit the baseline**

```bash
git commit -m "chore: baseline verified TS migration + tooling + Rust module split

Frontend migrated JS->TS (Pinia stores, composables, typed events),
adds eslint/prettier/lefthook/vitest/tsconfig; backend split into
audio/{chunked,constants,error,manager,stats,stream,wav_helper}.
Verified green: vitest 17, vue-tsc clean, eslint clean, cargo test 10."
```

- [ ] **Step 4: Confirm a clean tree**

Run: `git status --short`
Expected: empty (no output).

---

## Milestone 1 — DSCP/QoS fix (end-to-end)

### Task 1.1: Create the `transport` module skeleton

**Files:**

- Create: `src-tauri/src/audio/transport/mod.rs`
- Modify: `src-tauri/src/audio/mod.rs`

- [ ] **Step 1: Create `transport/mod.rs`**

```rust
//! Network transport: socket setup, DSCP marking, and (later) the Transport trait.

pub mod dscp;
```

- [ ] **Step 2: Wire it into `audio/mod.rs`**

In `src-tauri/src/audio/mod.rs`, add `pub mod transport;` alongside the existing `pub mod` lines (after `pub mod stream;`):

```rust
pub mod chunked;
pub mod commands;
pub mod constants;
pub mod engine;
pub mod error;
pub mod manager;
pub mod stats;
pub mod stream;
pub mod transport;
pub mod wav_helper;
```

> Note: `engine` is added here too; its `mod.rs` is created in Task 2.1. Until then, create an empty placeholder so the crate compiles: create `src-tauri/src/audio/engine/mod.rs` containing only `// engine submodules added in later tasks`. Replace it in Task 2.1.

- [ ] **Step 3: Add the engine placeholder so the crate compiles**

Create `src-tauri/src/audio/engine/mod.rs`:

```rust
// Engine submodules are added in later tasks.
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: compiles; `test result: ok. 10 passed`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/audio/mod.rs src-tauri/src/audio/transport/mod.rs src-tauri/src/audio/engine/mod.rs
git commit -m "chore(audio): scaffold engine and transport modules"
```

### Task 1.2: DSCP pure mapping (TDD)

**Files:**

- Create: `src-tauri/src/audio/transport/dscp.rs`

- [ ] **Step 1: Write the failing tests + function stub**

Create `src-tauri/src/audio/transport/dscp.rs`:

```rust
//! DSCP (Differentiated Services) strategy → IP TOS byte mapping.
//!
//! These keys are the single source of truth shared by the frontend dropdown
//! and the socket setup. Keys are case-insensitive. Unknown keys fall back to
//! best-effort (0x00) rather than failing.

/// Maps a DSCP strategy key to its IP TOS/DSCP byte value.
pub fn dscp_to_tos(strategy: &str) -> u8 {
    match strategy.to_ascii_lowercase().as_str() {
        "voip" | "ef" => 0xB8,      // Expedited Forwarding (DSCP 46)
        "cs5" => 0xA0,              // Class Selector 5 (DSCP 40)
        "lowdelay" => 0x10,         // Legacy "minimize delay"
        "throughput" => 0x08,       // Legacy "maximize throughput"
        "besteffort" | "" => 0x00,  // Best effort / default
        _ => 0x00,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voip_and_ef_map_to_expedited_forwarding() {
        assert_eq!(dscp_to_tos("voip"), 0xB8);
        assert_eq!(dscp_to_tos("ef"), 0xB8);
    }

    #[test]
    fn keys_are_case_insensitive() {
        assert_eq!(dscp_to_tos("VoIP"), 0xB8);
        assert_eq!(dscp_to_tos("LowDelay"), 0x10);
    }

    #[test]
    fn cs5_lowdelay_throughput() {
        assert_eq!(dscp_to_tos("cs5"), 0xA0);
        assert_eq!(dscp_to_tos("lowdelay"), 0x10);
        assert_eq!(dscp_to_tos("throughput"), 0x08);
    }

    #[test]
    fn besteffort_empty_and_unknown_are_zero() {
        assert_eq!(dscp_to_tos("besteffort"), 0x00);
        assert_eq!(dscp_to_tos(""), 0x00);
        assert_eq!(dscp_to_tos("nonsense"), 0x00);
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml dscp 2>&1 | tail -10`
Expected: 4 tests pass (the implementation is included above so they pass immediately — this is a pure function with no prior version).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/transport/dscp.rs
git commit -m "feat(transport): add tested DSCP->TOS mapping"
```

### Task 1.3: Apply DSCP in client AND server via the shared mapping

**Files:**

- Modify: `src-tauri/src/audio/stream.rs`

- [ ] **Step 1: Replace the inline client-side TOS match**

In `src-tauri/src/audio/stream.rs`, find the client-mode block (currently around lines 678-693):

```rust
                    let tos_val = match dscp_strategy.as_str() {
                        "EF" => 0xB8,
                        "CS5" => 0xA0,
                        "LowDelay" => 0x10,
                        _ => 0x00,
                    };

                    if tos_val > 0 {
                        if let Err(e) = socket.set_tos(tos_val) {
```

Replace the `match` with the shared function:

```rust
                    let tos_val = super::transport::dscp::dscp_to_tos(&dscp_strategy);

                    if tos_val > 0 {
                        if let Err(e) = socket.set_tos(tos_val) {
```

- [ ] **Step 2: Apply DSCP in server mode too**

In the server-mode accept block, right after `let _ = stream.set_nodelay(true);` (currently around line 594), add TOS marking on the accepted connection. `stream` is a `std::net::TcpStream`; wrap it via `socket2` to set TOS:

```rust
                                let _ = stream.set_nodelay(true);

                                // Apply DSCP/QoS to the accepted client connection (parity with client mode).
                                let tos_val = super::transport::dscp::dscp_to_tos(&format_clone /* placeholder */);
```

> CORRECTION — do not use `format_clone`. The DSCP strategy must be threaded into the network thread. The strategy string `dscp_strategy` is currently moved into the client branch only. Make it available to both branches:

1. Near the top of the network-thread closure (where `ip_clone`, `format_clone` are cloned, around lines 414-419), add: `let dscp_clone = dscp_strategy.clone();`
2. In the client branch, use `dscp_clone` instead of `dscp_strategy`.
3. In the server branch, after `set_nodelay`, add:

```rust
                                // Apply DSCP/QoS to the accepted client connection (parity with client mode).
                                let tos_val = super::transport::dscp::dscp_to_tos(&dscp_clone);
                                if tos_val > 0 {
                                    let sref = socket2::SockRef::from(&stream);
                                    if let Err(e) = sref.set_tos(tos_val) {
                                        emit_log(&app_handle_net, "warning", format!("Failed to set server QoS/TOS: {}", e));
                                    }
                                }
```

> `socket2::SockRef::from(&stream)` borrows the existing `TcpStream` without taking ownership, so the stream stays usable for writes.

- [ ] **Step 3: Verify it compiles and all tests pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8`
Expected: compiles (clippy-clean due to `#![deny(clippy::all)]`); all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/stream.rs
git commit -m "fix(audio): apply correct DSCP/TOS in client and server via shared mapping"
```

### Task 1.4: Lock the DSCP key contract on the frontend

**Files:**

- Modify: `src/components/tabs/AdvancedTab.vue` (verify only)
- Modify: `src/stores/__tests__/settings.test.ts`

- [ ] **Step 1: Confirm the dropdown values already match the backend keys**

Run: `grep -n 'value=' src/components/tabs/AdvancedTab.vue`
Expected: values are `voip`, `lowdelay`, `throughput`, `besteffort` — all accepted by `dscp_to_tos`. No change needed. If any value differs, change it to one of those keys.

- [ ] **Step 2: Add a guard test for the DSCP default**

In `src/stores/__tests__/settings.test.ts`, add a test asserting the store's default `dscpStrategy` is a key the backend understands (prevents regressing to a non-matching value like the old `"EF"`):

```ts
it("uses a DSCP strategy key the backend recognizes", () => {
  const store = useSettingsStore();
  const validKeys = ["voip", "ef", "cs5", "lowdelay", "throughput", "besteffort"];
  expect(validKeys).toContain(store.dscpStrategy);
});
```

> Match the existing test file's setup (Pinia `setActivePinia(createPinia())` in `beforeEach`). If the file lacks a settings-store test block, add the import `import { useSettingsStore } from "../settings";` and a `describe("settings store", () => { ... })` wrapper following the pattern already in `stream.test.ts`.

- [ ] **Step 2: Run the frontend tests**

Run: `npm test 2>&1 | tail -8`
Expected: all tests pass (18 total).

- [ ] **Step 3: Commit**

```bash
git add src/stores/__tests__/settings.test.ts src/components/tabs/AdvancedTab.vue
git commit -m "test(settings): lock DSCP strategy key contract"
```

---

## Milestone 2 — `engine/device.rs`: testable config selection

### Task 2.1: Define the engine module and pure config-ranking

**Files:**

- Modify: `src-tauri/src/audio/engine/mod.rs`
- Create: `src-tauri/src/audio/engine/device.rs`

- [ ] **Step 1: Replace the engine placeholder**

Overwrite `src-tauri/src/audio/engine/mod.rs`:

```rust
//! Audio engine: device selection, capture, and (later) buffering.

pub mod device;
```

- [ ] **Step 2: Write `device.rs` with the pure ranking function and its tests**

Create `src-tauri/src/audio/engine/device.rs`. The ranking mirrors the current preference (stereo > mono, then F32 > I16 > U16, requiring the requested sample rate to be in range). It operates on plain descriptors so it is testable without constructing cpal types.

```rust
//! Pure audio-config selection logic, decoupled from cpal so it can be tested.

/// Sample format candidates we support, in nothing-implied order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFmt {
    F32,
    I16,
    U16,
}

/// A simplified view of a `cpal::SupportedStreamConfigRange`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigCandidate {
    pub channels: u16,
    pub format: SampleFmt,
    pub min_rate: u32,
    pub max_rate: u32,
}

/// Picks the index of the best candidate for the requested sample rate.
///
/// Preference: stereo (2ch) before mono (1ch); within a channel count,
/// F32 before I16 before U16. Only candidates whose [min_rate, max_rate]
/// range contains `sample_rate` are eligible. Returns `None` if none match.
pub fn pick_best(candidates: &[ConfigCandidate], sample_rate: u32) -> Option<usize> {
    for ch in [2u16, 1u16] {
        for fmt in [SampleFmt::F32, SampleFmt::I16, SampleFmt::U16] {
            for (i, c) in candidates.iter().enumerate() {
                if c.channels == ch
                    && c.format == fmt
                    && c.min_rate <= sample_rate
                    && c.max_rate >= sample_rate
                {
                    return Some(i);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(channels: u16, format: SampleFmt, min_rate: u32, max_rate: u32) -> ConfigCandidate {
        ConfigCandidate { channels, format, min_rate, max_rate }
    }

    #[test]
    fn prefers_stereo_f32_when_available() {
        let cands = vec![
            c(1, SampleFmt::F32, 8000, 48000),
            c(2, SampleFmt::I16, 8000, 48000),
            c(2, SampleFmt::F32, 8000, 48000),
        ];
        assert_eq!(pick_best(&cands, 48000), Some(2));
    }

    #[test]
    fn falls_back_to_mono_when_no_stereo() {
        let cands = vec![c(1, SampleFmt::I16, 8000, 48000)];
        assert_eq!(pick_best(&cands, 48000), Some(0));
    }

    #[test]
    fn respects_sample_rate_range() {
        let cands = vec![
            c(2, SampleFmt::F32, 8000, 44100), // does not cover 48000
            c(2, SampleFmt::I16, 8000, 48000),
        ];
        assert_eq!(pick_best(&cands, 48000), Some(1));
    }

    #[test]
    fn returns_none_when_nothing_matches() {
        let cands = vec![c(2, SampleFmt::F32, 96000, 192000)];
        assert_eq!(pick_best(&cands, 48000), None);
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml engine::device 2>&1 | tail -10`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/engine/mod.rs src-tauri/src/audio/engine/device.rs
git commit -m "feat(engine): add tested pure audio-config selection"
```

### Task 2.2: Use `pick_best` from `stream.rs`

**Files:**

- Modify: `src-tauri/src/audio/stream.rs`

- [ ] **Step 1: Map cpal configs to candidates and call `pick_best`**

In `stream.rs`, the current selection loop is at lines 332-360 (the triple nested `for ch / for fmt / for range` plus the fallback). Replace that block (from `let mut selected_format = ...` through the fallback `if best_config_range.is_none()` block) with a mapping to `ConfigCandidate` + a call to `pick_best`:

```rust
    use super::engine::device::{pick_best, ConfigCandidate, SampleFmt};

    let to_fmt = |f: cpal::SampleFormat| match f {
        cpal::SampleFormat::F32 => Some(SampleFmt::F32),
        cpal::SampleFormat::I16 => Some(SampleFmt::I16),
        cpal::SampleFormat::U16 => Some(SampleFmt::U16),
        _ => None,
    };

    let candidates: Vec<ConfigCandidate> = supported_configs
        .iter()
        .filter_map(|r| {
            to_fmt(r.sample_format()).map(|format| ConfigCandidate {
                channels: r.channels(),
                format,
                min_rate: r.min_sample_rate().0,
                max_rate: r.max_sample_rate().0,
            })
        })
        .collect();

    let (config_range, selected_format) = match pick_best(&candidates, sample_rate) {
        Some(i) => {
            // Map back to the original cpal range. The candidate list is built
            // 1:1 from supported_configs (minus unsupported formats), so we find
            // the matching cpal range by its properties.
            let cand = candidates[i];
            let range = supported_configs
                .iter()
                .find(|r| {
                    to_fmt(r.sample_format()) == Some(cand.format)
                        && r.channels() == cand.channels
                        && r.min_sample_rate().0 == cand.min_rate
                        && r.max_sample_rate().0 == cand.max_rate
                })
                .copied()
                .expect("candidate originates from supported_configs");
            (range, range.sample_format())
        }
        None => {
            let fallback = *supported_configs
                .first()
                .ok_or_else(|| "No supported audio config found".to_string())?;
            emit_log(&app_handle, "warn", format!(
                "Requested Sample Rate {}Hz not natively supported. Relying on OS resampler.",
                sample_rate
            ));
            (fallback, fallback.sample_format())
        }
    };
    let device_channels = config_range.channels();
```

> This removes the old `let mut selected_format = cpal::SampleFormat::F32;` / `let mut best_config_range = None;` / nested loops / fallback. Keep the subsequent `emit_log("Audio Input: ...")` and `stream_config` construction that follow.

- [ ] **Step 2: Verify compile + all tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8`
Expected: compiles clean; all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/stream.rs
git commit -m "refactor(audio): select capture config via tested engine::device::pick_best"
```

---

## Milestone 3 — `engine/encoder.rs`: testable PCM encoding

### Task 3.1: Pure f32 → PCM i16 LE encoder (TDD)

**Files:**

- Modify: `src-tauri/src/audio/engine/mod.rs`
- Create: `src-tauri/src/audio/engine/encoder.rs`

- [ ] **Step 1: Declare the module**

In `src-tauri/src/audio/engine/mod.rs` add:

```rust
pub mod encoder;
```

- [ ] **Step 2: Write the encoder + tests**

Create `src-tauri/src/audio/engine/encoder.rs`:

```rust
//! Encoding of f32 audio samples into wire formats.

/// Encodes f32 samples (nominally in [-1.0, 1.0]) into signed 16-bit
/// little-endian PCM, replacing the contents of `out`. Out-of-range samples
/// are clamped before scaling. `out` keeps its capacity across calls, so a
/// reused buffer performs no allocation after warmup.
pub fn encode_f32_to_pcm_i16_le(samples: &[f32], out: &mut Vec<u8>) {
    out.clear();
    out.reserve(samples.len() * 2);
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let v = (clamped * 32767.0) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_known_values_little_endian() {
        let mut out = Vec::new();
        encode_f32_to_pcm_i16_le(&[0.0, 1.0, -1.0], &mut out);
        // 0 -> 0x0000, 1.0 -> 32767 (0x7FFF), -1.0 -> -32767 (0x8001)
        assert_eq!(out, vec![0x00, 0x00, 0xFF, 0x7F, 0x01, 0x80]);
    }

    #[test]
    fn clamps_out_of_range_input() {
        let mut out = Vec::new();
        encode_f32_to_pcm_i16_le(&[2.0, -2.0], &mut out);
        // clamps to 1.0 -> 32767 and -1.0 -> -32767
        assert_eq!(out, vec![0xFF, 0x7F, 0x01, 0x80]);
    }

    #[test]
    fn empty_input_yields_empty_output() {
        let mut out = vec![1, 2, 3];
        encode_f32_to_pcm_i16_le(&[], &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn output_len_is_two_bytes_per_sample() {
        let mut out = Vec::new();
        encode_f32_to_pcm_i16_le(&[0.1, 0.2, 0.3, 0.4], &mut out);
        assert_eq!(out.len(), 8);
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml engine::encoder 2>&1 | tail -10`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/engine/mod.rs src-tauri/src/audio/engine/encoder.rs
git commit -m "feat(engine): add tested f32->PCM i16 LE encoder"
```

### Task 3.2: Use the encoder in the send path

**Files:**

- Modify: `src-tauri/src/audio/stream.rs`

- [ ] **Step 1: Replace the inline conversion**

In `stream.rs`, the send path currently builds the payload inline (lines 786-793):

```rust
                if count > 0 {
                    // Convert float samples to PCM i16 payload
                    let mut payload = Vec::with_capacity(count * 2);
                    for &sample_f32 in temp_buffer.iter().take(count) {
                        let sample = sample_f32.clamp(-1.0, 1.0);
                        let sample_i16 = (sample * 32767.0) as i16;
                        payload.extend_from_slice(&sample_i16.to_le_bytes());
                    }
```

Replace with a reused buffer + the encoder. First, declare a reusable `payload` buffer once, before the `while is_running_clone...` loop (near line 521, next to `let mut prefill_debug_timer = ...`):

```rust
        let mut payload: Vec<u8> = Vec::new();
```

Then replace the inline conversion block with:

```rust
                if count > 0 {
                    super::engine::encoder::encode_f32_to_pcm_i16_le(&temp_buffer[..count], &mut payload);
```

> The rest of the block (the `let write_start = ...` and the match that writes `&payload`) is unchanged. Because `payload` is now declared outside the loop and reused, remove the old `let mut payload = Vec::with_capacity(count * 2);` line.

- [ ] **Step 2: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8`
Expected: compiles; all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/stream.rs
git commit -m "refactor(audio): encode PCM via tested encoder with a reused buffer"
```

---

## Milestone 4 — Real-time-safe capture (`engine/capture.rs`)

### Task 4.1: Add an `overruns` counter to `StreamStats`

**Files:**

- Modify: `src-tauri/src/audio/stats.rs`

- [ ] **Step 1: Add the field**

In `src-tauri/src/audio/stats.rs`, extend `StreamStats` (currently lines 42-48) to carry the overrun counter:

```rust
pub struct StreamStats {
    #[allow(dead_code)]
    pub bytes_sent: Arc<AtomicU64>,
    #[allow(dead_code)]
    pub start_time: Instant,
    pub is_running: Arc<AtomicBool>,
    /// Samples dropped because the ring buffer was full when the capture
    /// callback tried to push (real overrun signal). Used by metrics (Part B).
    pub overruns: Arc<AtomicU64>,
}
```

- [ ] **Step 2: Verify compile (expect an error at the `StreamStats { ... }` literal)**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8`
Expected: FAIL — `missing field 'overruns' in initializer of StreamStats` at `stream.rs`. That is fixed in Task 4.2.

> Do not commit yet — the crate does not compile until Task 4.2 wires the field.

### Task 4.2: RT-safe capture builder

**Files:**

- Create: `src-tauri/src/audio/engine/capture.rs`
- Modify: `src-tauri/src/audio/engine/mod.rs`
- Modify: `src-tauri/src/audio/stream.rs`

- [ ] **Step 1: Declare the module**

In `src-tauri/src/audio/engine/mod.rs` add:

```rust
pub mod capture;
```

- [ ] **Step 2: Write `capture.rs`**

Create `src-tauri/src/audio/engine/capture.rs`. This builds **only** the selected-format stream, moving the SPSC producer in (no `Arc<Mutex>`), and converts I16/U16 via a reusable scratch buffer (no per-callback allocation after warmup). It counts overruns when the ring buffer is full.

```rust
//! Real-time-safe cpal capture: the audio callback holds no lock and performs
//! no heap allocation after warmup.

use cpal::traits::DeviceTrait;
use cpal::{SampleFormat, StreamConfig};
use ringbuf::HeapProducer;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Builds an input stream for the given format, moving `producer` into the
/// callback. `overruns` is incremented by the number of samples dropped when
/// the ring buffer cannot accept the whole callback buffer.
pub fn build_input_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    format: SampleFormat,
    mut producer: HeapProducer<f32>,
    overruns: Arc<AtomicU64>,
    capacity_hint: usize,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let err_fn = |err: cpal::StreamError| {
        log::error!("CPAL stream error: {}", err);
    };

    match format {
        SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let pushed = producer.push_slice(data);
                if pushed < data.len() {
                    overruns.fetch_add((data.len() - pushed) as u64, Ordering::Relaxed);
                }
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => {
            let mut scratch: Vec<f32> = Vec::with_capacity(capacity_hint);
            device.build_input_stream(
                config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    scratch.clear();
                    scratch.extend(data.iter().map(|&s| s as f32 / 32768.0));
                    let pushed = producer.push_slice(&scratch);
                    if pushed < scratch.len() {
                        overruns.fetch_add((scratch.len() - pushed) as u64, Ordering::Relaxed);
                    }
                },
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let mut scratch: Vec<f32> = Vec::with_capacity(capacity_hint);
            device.build_input_stream(
                config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    scratch.clear();
                    scratch.extend(data.iter().map(|&s| (s as f32 - 32768.0) / 32768.0));
                    let pushed = producer.push_slice(&scratch);
                    if pushed < scratch.len() {
                        overruns.fetch_add((scratch.len() - pushed) as u64, Ordering::Relaxed);
                    }
                },
                err_fn,
                None,
            )
        }
        other => Err(cpal::BuildStreamError::StreamConfigNotSupported).map_err(|_| {
            log::error!("Unsupported sample format: {:?}", other);
            cpal::BuildStreamError::StreamConfigNotSupported
        }),
    }
}
```

- [ ] **Step 3: Replace the `Arc<Mutex>` stream-build block in `stream.rs`**

In `stream.rs`, the producer is currently created at line 406 (`let (prod, mut cons) = rb.split();`). Leave that. Remove the `err_fn` closure (lines 959-961) and the `let prod = Arc::new(Mutex::new(prod));` (line 964) and the whole `let audio_stream = match selected_format { ... }` block (lines 966-1009). Replace all of it with:

```rust
    let overruns = Arc::new(AtomicU64::new(0));
    let capacity_hint = chunk_size as usize * device_channels as usize;
    let audio_stream = super::engine::capture::build_input_stream(
        &device,
        &stream_config,
        selected_format,
        prod,
        overruns.clone(),
        capacity_hint,
    )
    .map_err(|e| e.to_string())?;
```

- [ ] **Step 4: Add `overruns` to the returned `StreamStats`**

At the end of `start_audio_stream` (the `Ok((audio_stream, StreamStats { ... }))` at lines 1011-1018), add the field:

```rust
    Ok((
        audio_stream,
        StreamStats {
            bytes_sent,
            start_time: Instant::now(),
            is_running,
            overruns,
        },
    ))
```

- [ ] **Step 5: Remove now-unused imports**

In `stream.rs`, remove `use std::sync::{mpsc, Arc, Mutex};` → change to `use std::sync::{mpsc, Arc};` if `Mutex` is no longer used elsewhere in the file (the producer mutex was the only use). Also remove `use log::error;` if `err_fn` was its only consumer. Run the compiler to confirm which imports are unused.

- [ ] **Step 6: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -10`
Expected: compiles clean (no `unused import`, clippy-clean); all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/audio/engine/mod.rs src-tauri/src/audio/engine/capture.rs src-tauri/src/audio/stream.rs src-tauri/src/audio/stats.rs
git commit -m "fix(audio): real-time-safe capture (no mutex/alloc in callback) + overrun counter"
```

---

## Milestone 5 — Effective "Buffer Size" (cpal `BufferSize::Fixed`)

### Task 5.1: Pure buffer-size negotiation (TDD)

**Files:**

- Modify: `src-tauri/src/audio/engine/capture.rs`

- [ ] **Step 1: Add the negotiation function + tests to `capture.rs`**

Append to `src-tauri/src/audio/engine/capture.rs`:

```rust
use cpal::{BufferSize, SupportedBufferSize};

/// Resolves a requested hardware buffer size (in frames) against the device's
/// supported range. Out-of-range requests are clamped; `Unknown` ranges and a
/// zero/unset request fall back to the driver default.
pub fn resolve_buffer_size(requested: u32, supported: &SupportedBufferSize) -> BufferSize {
    match supported {
        SupportedBufferSize::Range { min, max } => {
            if requested == 0 {
                BufferSize::Default
            } else {
                BufferSize::Fixed(requested.clamp(*min, *max))
            }
        }
        SupportedBufferSize::Unknown => BufferSize::Default,
    }
}

#[cfg(test)]
mod buffer_size_tests {
    use super::*;
    use cpal::{BufferSize, SupportedBufferSize};

    #[test]
    fn within_range_is_used_verbatim() {
        let s = SupportedBufferSize::Range { min: 64, max: 4096 };
        assert!(matches!(resolve_buffer_size(1024, &s), BufferSize::Fixed(1024)));
    }

    #[test]
    fn below_min_clamps_up() {
        let s = SupportedBufferSize::Range { min: 256, max: 4096 };
        assert!(matches!(resolve_buffer_size(64, &s), BufferSize::Fixed(256)));
    }

    #[test]
    fn above_max_clamps_down() {
        let s = SupportedBufferSize::Range { min: 64, max: 2048 };
        assert!(matches!(resolve_buffer_size(8192, &s), BufferSize::Fixed(2048)));
    }

    #[test]
    fn unknown_range_falls_back_to_default() {
        assert!(matches!(
            resolve_buffer_size(1024, &SupportedBufferSize::Unknown),
            BufferSize::Default
        ));
    }

    #[test]
    fn zero_request_falls_back_to_default() {
        let s = SupportedBufferSize::Range { min: 64, max: 4096 };
        assert!(matches!(resolve_buffer_size(0, &s), BufferSize::Default));
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml resolve_buffer_size 2>&1 | tail -10`
Expected: 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/engine/capture.rs
git commit -m "feat(engine): add tested cpal buffer-size negotiation"
```

### Task 5.2: Thread `buffer_size` through and apply it

**Files:**

- Modify: `src-tauri/src/audio/commands.rs`
- Modify: `src-tauri/src/audio/manager.rs`
- Modify: `src-tauri/src/audio/stream.rs`

- [ ] **Step 1: Stop discarding `buffer_size` in the command**

In `src-tauri/src/audio/commands.rs`, remove the discard (line 42 `let _ = buffer_size;`) and pass `buffer_size` into the `AudioCommand::Start { ... }` it builds. Add `buffer_size,` to that struct literal.

- [ ] **Step 2: Add `buffer_size` to `AudioCommand::Start` and `StreamParams`**

In `src-tauri/src/audio/manager.rs`:

- Add `buffer_size: u32,` to the `AudioCommand::Start { ... }` variant (near line 24).
- Add `buffer_size: u32,` to `struct StreamParams` (near line 47).
- In the `Start` match arm, destructure `buffer_size,` and include it when building `StreamParams` and when calling `start_audio_stream(...)`.
- In the reconnect call (the `start_audio_stream(p.device_name.clone(), ...)`), pass `p.buffer_size`.
- Add `buffer_size: u32,` as a parameter to the `start_audio_stream` signature in `stream.rs` (it is declared in `stream.rs`); thread it through both call sites in `manager.rs`.

> Place `buffer_size` immediately after `sample_rate` in every signature/struct/call for consistency.

- [ ] **Step 3: Apply it in `stream.rs` when building `stream_config`**

In `stream.rs`, the `stream_config` is currently built with `buffer_size: cpal::BufferSize::Default` (line 374). Replace with the negotiated value:

```rust
    let stream_config = cpal::StreamConfig {
        channels: device_channels,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: super::engine::capture::resolve_buffer_size(
            buffer_size,
            config_range.buffer_size(),
        ),
    };
```

Add a log line right after so the resolved size is visible:

```rust
    emit_log(&app_handle, "info", format!(
        "Capture buffer: requested {} frames -> {:?}",
        buffer_size, stream_config.buffer_size
    ));
```

- [ ] **Step 4: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8`
Expected: compiles; all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/audio/commands.rs src-tauri/src/audio/manager.rs src-tauri/src/audio/stream.rs
git commit -m "fix(audio): make Buffer Size control actually set the cpal buffer size"
```

---

## Milestone 6 — Verification & UI label clarity

### Task 6.1: Clarify buffer-related labels (minimal, no redesign)

**Files:**

- Modify: `src/components/tabs/AudioTab.vue`

- [ ] **Step 1: Relabel "Buffer Size" to reflect that it is the hardware capture buffer**

In `src/components/tabs/AudioTab.vue`, the `Buffer Size` select (lines 18-28) controls the cpal capture buffer. Change its `label` from `"Buffer Size"` to `"Capture Buffer (frames)"` and the `Ring Buffer (ms)` select label (line 34) to `"Jitter Buffer (ms)"` so the two are clearly different concepts. Do not change option values.

- [ ] **Step 2: Run frontend checks**

Run: `npm test && npm run typecheck && npm run lint`
Expected: all green.

- [ ] **Step 3: Commit**

```bash
git add src/components/tabs/AudioTab.vue
git commit -m "ui(audio): clarify capture-buffer vs jitter-buffer labels"
```

### Task 6.2: Full green gate + manual smoke test

- [ ] **Step 1: Run the complete check suite**

Run:

```bash
npm test && npm run typecheck && npm run lint && \
cargo test --manifest-path src-tauri/Cargo.toml && \
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: vitest green; vue-tsc clean; eslint clean; `cargo test` ~30 tests pass; clippy clean.

- [ ] **Step 2: Manual smoke test (client + server)**

Run: `npm run tauri dev`
Then verify:

1. Devices load; pick an input device.
2. Server mode: Start Listening; from another machine/VLC open `http://<LAN_IP>:<port>/stream.wav` → audio plays; logs show "Capture buffer: requested ... -> Fixed(...)" (or a Default fallback with a clear reason).
3. Client mode against a Snapserver TCP source: Start Streaming → "Connected"; audio reaches Snapcast.
4. Advanced tab: switch DSCP to VoIP; logs show no QoS warning (TOS applied). On Linux, optionally confirm with `tcpdump -v` that the DSCP bits are set.

Expected: all four behave as described, no regressions vs the pre-refactor app.

- [ ] **Step 3: Final commit (if any smoke-test tweaks were needed)**

```bash
git add -A
git commit -m "chore: Phase 1 Part A verification fixes"
```

---

## Self-Review (author)

- **Spec coverage (Part A):** §6 Fase 0 → M0; §5.5 DSCP → M1; §4 device → M2; §4 encoder → M3; §5.1 RT-safe → M4; §5.3 buffer size → M5; §5.8 tests → M1–M5; §5.9 (DSCP keys + buffer labels) → M1.4 + M6.1. Adaptive (§5.2), metrics (§5.4), reconnection (§5.6), HTTP handshake (§5.7) are explicitly Part B/C — not gaps.
- **Type consistency:** `dscp_to_tos(&str)->u8`, `pick_best(&[ConfigCandidate],u32)->Option<usize>`, `encode_f32_to_pcm_i16_le(&[f32],&mut Vec<u8>)`, `build_input_stream(&Device,&StreamConfig,SampleFormat,HeapProducer<f32>,Arc<AtomicU64>,usize)`, `resolve_buffer_size(u32,&SupportedBufferSize)->BufferSize`. `StreamStats.overruns: Arc<AtomicU64>` is set in M4 and consumed in Part B. `buffer_size` is placed after `sample_rate` everywhere.
- **No placeholders:** the only `/* placeholder */` is shown inside an explicit CORRECTION block in Task 1.3 that immediately tells the engineer not to use it and gives the real code.

## Next plans (to be written before executing each)

- **Part B — Real adaptive buffer + honest metrics:** `engine/buffer.rs` target-occupancy controller (pure + tests), wire into the consumer drain loop; `metrics.rs` underrun/overrun counters + quality scoring (pure + tests) + per-platform `TCP_INFO` RTT (`libc`/`windows-sys`, best-effort); new `QualityEvent`/`StatsEvent` shapes + `src/types/events.ts` + store + `StatsBar` labels.
- **Part C — Transport extraction + reliability:** `transport/mod.rs` `Transport` trait, `tcp_client.rs`/`tcp_server.rs`; non-blocking HTTP handshake (remove the 1.5s stall); consolidate the dual reconnection into one mechanism; dissolve the remainder of `stream.rs` into `engine/mod.rs` orchestration.
