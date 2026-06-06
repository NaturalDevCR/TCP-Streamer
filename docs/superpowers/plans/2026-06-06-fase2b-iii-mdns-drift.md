# Phase 2B-iii: mDNS Discovery + Clock-Drift Handling — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a native source advertise itself on the LAN via mDNS so a sink can discover it (no IP typing), keeping manual entry as a fallback; and handle source/sink clock drift (no PTP) by dropping or inserting a mini-chunk when the playback buffer trends away from target.

**Architecture:** A `transport/discovery.rs` wrapping `mdns-sd` (advertise on the source, browse on the sink → `list_sources` command). A pure, tested `DriftController` (EMA of buffered frames + hysteresis + cooldown → Drop / InsertSilence / None) wired into the sink's playback feed.

**Tech Stack:** Rust; `mdns-sd 0.11`, `local-ip-address` (already a dep). Vue 3.

**Spec:** `docs/superpowers/specs/2026-06-06-fase2b-modo-nativo-design.md` §5.6, §5.7, §3.3.
**Depends on:** 2B-i (+ ideally 2B-ii) committed. **Anchor edits by quoted code.**

---

## File Structure

| File                                                         | Responsibility                          | Status |
| ------------------------------------------------------------ | --------------------------------------- | ------ |
| `src-tauri/Cargo.toml`                                       | add `mdns-sd = "0.11"`                  | Modify |
| `src-tauri/src/audio/transport/discovery.rs`                 | mDNS advertise (source) + browse (sink) | Create |
| `src-tauri/src/audio/transport/mod.rs`                       | `pub mod discovery;`                    | Modify |
| `src-tauri/src/audio/transport/udp/drift.rs`                 | **(pure)** drift controller             | Create |
| `src-tauri/src/audio/transport/udp/mod.rs`                   | `pub mod drift;`                        | Modify |
| `src-tauri/src/audio/transport/udp/sink.rs`                  | apply drift correction in the feed      | Modify |
| `src-tauri/src/audio/commands.rs` · `lib.rs`                 | `list_sources` command                  | Modify |
| `src-tauri/src/audio/engine/mod.rs`                          | advertise when native source starts     | Modify |
| `src/stores/settings.ts` · `stream.ts` · `ConnectionTab.vue` | discovered-sources picker               | Modify |

---

## Milestone 1 — Clock-drift controller (pure, TDD)

### Task 1.1: `transport/udp/drift.rs`

**Files:** Create `src-tauri/src/audio/transport/udp/drift.rs`; Modify `src-tauri/src/audio/transport/udp/mod.rs`

- [ ] **Step 1: Declare the module**

In `transport/udp/mod.rs`, add `pub mod drift;`.

- [ ] **Step 2: Write the module + tests**

Create `src-tauri/src/audio/transport/udp/drift.rs`:

```rust
//! Clock-drift controller (no PTP). Tracks an EMA of the playback buffer level;
//! when it trends above target it asks to drop a mini-chunk (sink clock slower
//! than source), below target it asks to insert silence (sink faster). A
//! cooldown keeps corrections rare so they are inaudible micro-glitches.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftAction {
    None,
    DropChunk,
    InsertSilence,
}

pub struct DriftController {
    target: f32,
    margin: f32,
    alpha: f32,
    ema: f32,
    cooldown: u32,
    since: u32,
    primed: bool,
}

impl DriftController {
    /// `target`/`margin` are in the same unit you pass to `observe` (e.g. buffered
    /// frames). `cooldown` ticks must pass between corrections.
    pub fn new(target: f32, margin: f32, cooldown: u32) -> Self {
        Self { target, margin, alpha: 0.05, ema: target, cooldown, since: 0, primed: false, }
    }

    /// Feed the current buffered level; returns the correction to apply.
    pub fn observe(&mut self, buffered: f32) -> DriftAction {
        self.ema = if self.primed {
            self.alpha * buffered + (1.0 - self.alpha) * self.ema
        } else {
            self.primed = true;
            buffered
        };
        if self.since < self.cooldown {
            self.since += 1;
            return DriftAction::None;
        }
        if self.ema > self.target + self.margin {
            self.since = 0;
            DriftAction::DropChunk
        } else if self.ema < self.target - self.margin {
            self.since = 0;
            DriftAction::InsertSilence
        } else {
            DriftAction::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive(c: &mut DriftController, level: f32, n: usize) -> DriftAction {
        let mut last = DriftAction::None;
        for _ in 0..n {
            last = c.observe(level);
        }
        last
    }

    #[test]
    fn high_buffer_asks_to_drop() {
        let mut c = DriftController::new(100.0, 20.0, 2);
        assert_eq!(drive(&mut c, 200.0, 200), DriftAction::DropChunk);
    }

    #[test]
    fn low_buffer_asks_to_insert() {
        let mut c = DriftController::new(100.0, 20.0, 2);
        assert_eq!(drive(&mut c, 10.0, 200), DriftAction::InsertSilence);
    }

    #[test]
    fn on_target_does_nothing() {
        let mut c = DriftController::new(100.0, 20.0, 2);
        assert_eq!(drive(&mut c, 100.0, 200), DriftAction::None);
    }

    #[test]
    fn cooldown_prevents_back_to_back_corrections() {
        let mut c = DriftController::new(100.0, 20.0, 5);
        // First correction fires once ema crosses; immediately after, cooldown holds.
        let _ = drive(&mut c, 200.0, 200);
        assert_eq!(c.observe(200.0), DriftAction::None); // within cooldown
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml transport::udp::drift 2>&1 | tail -8`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/transport/udp/mod.rs src-tauri/src/audio/transport/udp/drift.rs
git commit -m "feat(udp): tested clock-drift controller (no PTP)"
```

### Task 1.2: Apply drift correction in the sink feed

**Files:** Modify `src-tauri/src/audio/transport/udp/sink.rs`

> 2B-i's `receive_loop` drains the jitter buffer and `producer.push_slice(&decoded)` into the playback ring. The drift signal is the playback ring's fill vs its capacity. The simplest in-loop signal is `producer.len()` (ringbuf producer exposes occupancy).

- [ ] **Step 1: Observe occupancy and correct**

Add `let mut drift = super::drift::DriftController::new(target_frames as f32, (target_frames / 4) as f32, 200);` where `target_frames` is passed in (the orchestrator computes it from the latency profile). Once per loop iteration, after draining the jitter buffer, observe and act:

```rust
            match drift.observe(producer.len() as f32) {
                super::drift::DriftAction::DropChunk => {
                    // sink slower than source: discard a tiny amount to catch up
                    let drop = (target_frames / 20).max(8);
                    for _ in 0..drop { if producer.pop().is_none() { break; } }
                }
                super::drift::DriftAction::InsertSilence => {
                    let ins = (target_frames / 20).max(8);
                    for _ in 0..ins { let _ = producer.push(0.0); }
                }
                super::drift::DriftAction::None => {}
            }
```

> `HeapProducer` in `ringbuf 0.3` exposes `len()`, `push(value)`, and `pop()` is on the consumer — for the producer side, "dropping" is better done by not pushing; if `producer` lacks `pop`, instead skip pushing `drop` samples on the next decode (track a `skip` counter). Adjust to the available API: the robust form is a `skip: usize` counter decremented as you push decoded samples, and an `insert: usize` counter that pushes zeros. Implement whichever the compiler accepts; keep behavior: DropChunk → skip N upcoming samples, InsertSilence → push N zeros.

- [ ] **Step 2: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: compiles; all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/transport/udp/sink.rs
git commit -m "feat(udp): apply clock-drift correction in the sink feed"
```

---

## Milestone 2 — mDNS discovery

### Task 2.1: Add `mdns-sd`

**Files:** Modify `src-tauri/Cargo.toml`

- [ ] **Step 1: Add the dependency**

In `[dependencies]`, add: `mdns-sd = "0.11"`.

- [ ] **Step 2: Verify + commit**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -3`

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "build: add mdns-sd for native-source discovery"
```

### Task 2.2: `transport/discovery.rs`

**Files:** Create `src-tauri/src/audio/transport/discovery.rs`; Modify `src-tauri/src/audio/transport/mod.rs`

- [ ] **Step 1: Declare the module**

In `transport/mod.rs`, add `pub mod discovery;`.

- [ ] **Step 2: Write the module**

Create `src-tauri/src/audio/transport/discovery.rs` (API per `mdns-sd 0.11`: `ServiceDaemon::new()`, `register(ServiceInfo)`, `browse(ty) -> Receiver<ServiceEvent>`, `ServiceEvent::ServiceResolved(ServiceInfo)`):

```rust
//! mDNS advertise (source) + browse (sink) for native UDP sources.

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::Serialize;
use std::time::{Duration, Instant};

const SERVICE_TYPE: &str = "_tcp-streamer._udp.local.";

/// Keeps the mDNS registration alive while held (drop to unregister).
pub struct Advertiser {
    _daemon: ServiceDaemon,
}

/// Advertises this instance as a native source on the LAN.
pub fn advertise(instance: &str, port: u16, encrypted: bool) -> Result<Advertiser, String> {
    let daemon = ServiceDaemon::new().map_err(|e| e.to_string())?;
    let ip = local_ip_address::local_ip().map_err(|e| e.to_string())?;
    let host = format!("{instance}.local.");
    let enc = if encrypted { "1" } else { "0" };
    let props = [("ver", "1"), ("enc", enc)];
    let info = ServiceInfo::new(SERVICE_TYPE, instance, &host, ip, port, &props[..])
        .map_err(|e| e.to_string())?;
    daemon.register(info).map_err(|e| e.to_string())?;
    Ok(Advertiser { _daemon: daemon })
}

/// A discovered native source.
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredSource {
    pub name: String,
    pub addr: String, // ip:port
    pub encrypted: bool,
}

/// Browses for native sources for `timeout`, returning what resolved.
pub fn browse(timeout: Duration) -> Vec<DiscoveredSource> {
    let Ok(daemon) = ServiceDaemon::new() else { return Vec::new(); };
    let Ok(rx) = daemon.browse(SERVICE_TYPE) else { return Vec::new(); };
    let deadline = Instant::now() + timeout;
    let mut out = Vec::new();
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(remaining) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                if let Some(ip) = info.get_addresses_v4().iter().next() {
                    let encrypted = info
                        .get_property_val_str("enc")
                        .map(|v| v == "1")
                        .unwrap_or(false);
                    out.push(DiscoveredSource {
                        name: info.get_fullname().to_string(),
                        addr: format!("{}:{}", ip, info.get_port()),
                        encrypted,
                    });
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    out
}
```

> Method names per `mdns-sd 0.11.5`: `ServiceInfo::{get_fullname, get_addresses_v4, get_port, get_property_val_str}`. If `get_property_val_str` differs in the resolved version, use the version's TXT-property accessor.

- [ ] **Step 3: Verify compile**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/transport/mod.rs src-tauri/src/audio/transport/discovery.rs
git commit -m "feat(transport): mDNS advertise + browse for native sources"
```

### Task 2.3: `list_sources` command + advertise on source start

**Files:** Modify `commands.rs`, `lib.rs`, `engine/mod.rs`

- [ ] **Step 1: Command**

In `commands.rs`:

```rust
#[tauri::command]
pub fn list_sources() -> Vec<crate::audio::transport::discovery::DiscoveredSource> {
    crate::audio::transport::discovery::browse(std::time::Duration::from_millis(1500))
}
```

Register `audio::list_sources,` in `lib.rs`'s `generate_handler!`.

- [ ] **Step 2: Advertise when the native source starts**

In `engine::run`'s UDP branch, after binding the `UdpSource`, advertise once and keep the `Advertiser` alive for the thread's lifetime:

```rust
                if advertiser.is_none() {
                    let name = format!("tcp-streamer-{}", port);
                    match super::transport::discovery::advertise(&name, port, !psk.is_empty()) {
                        Ok(a) => { advertiser = Some(a); emit_log(&app_handle_net, "info", "Advertised via mDNS".to_string()); }
                        Err(e) => emit_log(&app_handle_net, "warning", format!("mDNS advertise failed: {}", e)),
                    }
                }
```

Declare `let mut advertiser: Option<super::transport::discovery::Advertiser> = None;` near the UDP-source local. (`psk` is available if 2B-ii landed; otherwise pass `false`.)

- [ ] **Step 3: Verify + commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`

```bash
git add src-tauri/src/audio/commands.rs src-tauri/src/lib.rs src-tauri/src/audio/engine/mod.rs
git commit -m "feat(audio): list_sources command + advertise native source via mDNS"
```

---

## Milestone 3 — Frontend source picker

### Task 3.1: Discovered-sources dropdown

**Files:** Modify `src/stores/settings.ts`, `src/components/tabs/ConnectionTab.vue`

- [ ] **Step 1: Store action**

In `settings.ts`, add `const discoveredSources = ref<{name:string;addr:string;encrypted:boolean}[]>([]);` and:

```ts
async function scanSources() {
  try {
    discoveredSources.value = (await invoke("list_sources")) as {
      name: string;
      addr: string;
      encrypted: boolean;
    }[];
  } catch {
    discoveredSources.value = [];
  }
}
```

Export both.

- [ ] **Step 2: UI**

In `ConnectionTab.vue`, when `settings.role === 'sink'`, add a "Scan" button (`@click="settings.scanSources()"`) and, if `settings.discoveredSources.length`, a `SelectField` listing them (`:value="s.addr"`, label `{{ s.name }} {{ s.encrypted ? '🔒' : '' }}`) whose selection sets `settings.sourceAddr`. The manual `sourceAddr` input stays.

- [ ] **Step 3: Verify**

Run: `pnpm typecheck && pnpm lint && pnpm test && pnpm format:check 2>&1 | tail -4`
Expected: green.

- [ ] **Step 4: Commit**

```bash
git add src/stores/settings.ts src/components/tabs/ConnectionTab.vue
git commit -m "feat(ui): scan + pick discovered native sources (mDNS)"
```

---

## Milestone 4 — Verification

### Task 4.1: Full gate + manual

- [ ] **Step 1: Complete check suite**

Run:

```bash
pnpm test && pnpm typecheck && pnpm lint && pnpm format:check && \
cargo test --manifest-path src-tauri/Cargo.toml && \
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: all green; Rust tests up by ~4 (drift).

- [ ] **Step 2: Manual (two instances)**

Start a native source on A. On B (sink), click **Scan** → A appears in the list (🔒 if encrypted); pick it → `sourceAddr` fills → Start → audio plays. Let it run several minutes → confirm no slow buffer overrun/underrun (drift correction holds), only rare micro-glitches.

- [ ] **Step 3: Final commit (if needed)**

```bash
git add -A && git commit -m "chore: Phase 2B-iii verification fixes"
```

---

## Self-Review (author)

- **Spec coverage:** §5.6 mDNS (advertise + browse + `list_sources` + UI) → M2 + M3; §5.7 clock-drift → M1; §3.3 limitations (manual fallback kept; drift micro-glitch) honored.
- **Type consistency:** `DriftController::{new(f32,f32,u32), observe(f32)->DriftAction}`; `discovery::{advertise(&str,u16,bool)->Result<Advertiser,String>, browse(Duration)->Vec<DiscoveredSource>}`; `DiscoveredSource{name,addr,encrypted}` is `Serialize` and matches the TS shape used in `list_sources`/`scanSources`.
- **No placeholders:** complete code; two explicit version-API caveats (ringbuf producer drop method in M1.2; `mdns-sd` TXT accessor in M2.2), each with the exact intended behavior and a concrete fallback — not vague.

## Phase 2B complete after this plan

With 2B-i + 2B-ii + 2B-iii, the native mode is done: sink playback, UDP transport, optional AEAD/PSK encryption, mDNS discovery, and drift handling. Remaining for the whole initiative: merge the Phase-2 branches to `main` + version bump, then **Phase 3 (UI/UX redesign)** — its own spec → plan.
