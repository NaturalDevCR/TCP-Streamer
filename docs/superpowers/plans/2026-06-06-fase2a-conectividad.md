# Phase 2A: Connectivity & Configurable Latency — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace hardcoded buffer floors with a configurable latency↔robustness profile, support hostnames/IPv6 (client) and dual-stack bind (server), add an optional IP/CIDR allowlist, and remove vendor-specific naming so the app reads as a generic audio-receiver streamer.

**Architecture:** Three new pure, unit-tested modules — `engine/latency.rs` (profile→buffer params), `transport/resolve.rs` (host→SocketAddrs), `transport/allowlist.rs` (IP/CIDR matching) — plus thin wiring into `engine::run`, `tcp_client::connect`, and the command/settings chain. Single-client server is preserved.

**Tech Stack:** Rust (Tauri 2), `socket2 0.5`, `ipnet 2`, `std::net`; Vue 3 + Pinia + Vitest.

**Spec:** `docs/superpowers/specs/2026-06-06-fase2a-conectividad-design.md`
**Depends on:** Phase 1 merged (engine::run + transport modules exist). **Anchor edits by quoted code; line numbers drift.**

**Param-threading reference (a new `start_stream` arg touches all of these):**
`src/stores/stream.ts` (invoke) → `commands.rs::start_stream` → `manager.rs::AudioCommand::Start` (variant + destructure + `engine::run(...)` call) → `engine::run` signature.

---

## File Structure

| File | Responsibility | Status |
|---|---|---|
| `src-tauri/Cargo.toml` | add `ipnet = "2"` | Modify |
| `src-tauri/src/audio/engine/latency.rs` | **(pure)** latency profile → buffer params | Create |
| `src-tauri/src/audio/transport/resolve.rs` | **(pure*)** host/IPv4/IPv6 → SocketAddrs | Create |
| `src-tauri/src/audio/transport/allowlist.rs` | **(pure)** IP/CIDR parse + match | Create |
| `src-tauri/src/audio/engine/mod.rs` | use latency params; dual-stack bind; allowlist check | Modify |
| `src-tauri/src/audio/transport/tcp_client.rs` | connect via resolved addrs (hostnames/IPv6) | Modify |
| `src-tauri/src/audio/transport/mod.rs` | `pub mod resolve; pub mod allowlist;` | Modify |
| `src-tauri/src/audio/constants.rs` | remove hardcoded buffer floors | Modify |
| `src-tauri/src/audio/commands.rs` · `manager.rs` | thread `latency_profile`, `allowlist` | Modify |
| `src/stores/settings.ts` · `stream.ts` | latency profile + allowlist state/invoke | Modify |
| `src/components/tabs/AdvancedTab.vue` · `ConnectionTab.vue` · `AudioTab.vue` | latency selector, allowlist field, generic labels | Modify |
| `README.md` · `CONTRIBUTING.md` | generic terminology | Modify |

---

## Milestone 0 — Dependency

### Task 0.1: Add the `ipnet` crate

**Files:** Modify `src-tauri/Cargo.toml`

- [ ] **Step 1: Add the dependency**

In `src-tauri/Cargo.toml`, under `[dependencies]` (after `anyhow = "1"`), add:
```toml
ipnet = "2"
```

- [ ] **Step 2: Verify it resolves**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -3`
Expected: builds (ipnet 2.12 is cached).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "build: add ipnet for CIDR allowlist"
```

---

## Milestone 1 — Configurable latency profile

### Task 1.1: Pure `engine/latency.rs` (TDD)

**Files:** Create `src-tauri/src/audio/engine/latency.rs`; Modify `src-tauri/src/audio/engine/mod.rs`

- [ ] **Step 1: Declare the module**

In `src-tauri/src/audio/engine/mod.rs`, add to the `pub mod` block at the top:
```rust
pub mod latency;
```

- [ ] **Step 2: Write the module + tests**

Create `src-tauri/src/audio/engine/latency.rs`:

```rust
//! Latency profile → buffer parameters (pure, testable).
//!
//! Replaces the previous hardcoded buffer floors. Each named profile trades
//! latency against jitter tolerance; loopback (WASAPI) capture gets higher
//! floors because it needs more buffering. `prefill_ms` equals `ring_ms`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencyParams {
    pub ring_ms: u32,
    pub adaptive_min_ms: u32,
    pub adaptive_max_ms: u32,
    pub chunk_size: u32,
    pub prefill_ms: u32,
}

/// Returns buffer parameters for a named profile. Unknown profiles fall back to
/// "balanced". Use this only for the named profiles; "custom" is handled by the
/// caller, which passes the user's manual fields instead.
pub fn params(profile: &str, is_loopback: bool) -> LatencyParams {
    let (ring, amin, amax, chunk) = match (profile, is_loopback) {
        ("ultra-low", false) => (100, 50, 300, 256),
        ("ultra-low", true) => (600, 400, 1500, 256),
        ("robust", false) => (3000, 2000, 8000, 1024),
        ("robust", true) => (5000, 4000, 12000, 1024),
        // "balanced" and any unknown profile
        (_, false) => (500, 300, 2000, 512),
        (_, true) => (1500, 1000, 4000, 512),
    };
    LatencyParams {
        ring_ms: ring,
        adaptive_min_ms: amin,
        adaptive_max_ms: amax,
        chunk_size: chunk,
        prefill_ms: ring,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ultra_low_is_lowest_latency() {
        assert_eq!(params("ultra-low", false).ring_ms, 100);
        assert_eq!(params("ultra-low", false).chunk_size, 256);
    }

    #[test]
    fn robust_buffers_more_than_balanced() {
        assert!(params("robust", false).ring_ms > params("balanced", false).ring_ms);
    }

    #[test]
    fn unknown_profile_falls_back_to_balanced() {
        assert_eq!(params("nonsense", false), params("balanced", false));
    }

    #[test]
    fn loopback_floors_are_higher_than_non_loopback() {
        for p in ["ultra-low", "balanced", "robust"] {
            assert!(
                params(p, true).ring_ms >= params(p, false).ring_ms,
                "loopback ring for {p} should be >= non-loopback"
            );
            assert!(params(p, true).adaptive_min_ms >= params(p, false).adaptive_min_ms);
        }
    }

    #[test]
    fn prefill_equals_ring() {
        let lp = params("balanced", false);
        assert_eq!(lp.prefill_ms, lp.ring_ms);
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml engine::latency 2>&1 | tail -8`
Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/engine/mod.rs src-tauri/src/audio/engine/latency.rs
git commit -m "feat(engine): add tested latency profile -> buffer params"
```

### Task 1.2: Thread `latency_profile` through the command chain

**Files:** Modify `commands.rs`, `manager.rs`

- [ ] **Step 1: Add the param to `start_stream`**

In `src-tauri/src/audio/commands.rs`, add `latency_profile: String,` to the `start_stream` signature (after `max_buffer_ms: u32,`) and add `latency_profile,` to the `AudioCommand::Start { ... }` it constructs.

- [ ] **Step 2: Add the field to `AudioCommand::Start`**

In `src-tauri/src/audio/manager.rs`:
- Add `latency_profile: String,` to the `AudioCommand::Start { ... }` variant (after `max_buffer_ms: u32,`).
- Add `latency_profile,` to the destructuring pattern in the `Ok(AudioCommand::Start { ... })` arm.
- Add `latency_profile,` to the `super::engine::run(...)` call argument list, placed immediately after `max_buffer_ms,` and before `(*app_handle).clone(),`.

- [ ] **Step 3: Verify (expect engine::run arity error until Task 1.3)**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: FAIL — `engine::run` takes N arguments but N+1 supplied. Fixed in Task 1.3. Do not commit yet.

### Task 1.3: Use the profile in `engine::run`

**Files:** Modify `src-tauri/src/audio/engine/mod.rs`

- [ ] **Step 1: Add the parameter**

Add `latency_profile: String,` to the `pub fn run(...)` signature, immediately after `max_buffer_ms: u32,`.

- [ ] **Step 2: Resolve the buffer params at the top of the buffer setup**

Find the `// 1. Setup Ring Buffer` block. Replace the block that computes `adjusted_ring_buffer_duration_ms`, `ring_capacity_ms`, and `adj_ms_u32` (the `if is_loopback { LOOPBACK_MIN_BUFFER_MS.max(...) } ...` through `let adj_ms_u32 = ...;`) with profile resolution:

```rust
    // 1. Setup Ring Buffer (latency profile drives the sizes; "custom" uses the
    // user's manual fields).
    let lp = if latency_profile == "custom" {
        self::latency::LatencyParams {
            ring_ms: ring_buffer_duration_ms,
            adaptive_min_ms: min_buffer_ms,
            adaptive_max_ms: max_buffer_ms,
            chunk_size,
            prefill_ms: ring_buffer_duration_ms,
        }
    } else {
        self::latency::params(&latency_profile, is_loopback)
    };
    let effective_chunk = lp.chunk_size;

    let ring_capacity_ms = lp.adaptive_max_ms.max(lp.ring_ms);
    let ring_buffer_size =
        (sample_rate as usize) * (device_channels as usize) * (ring_capacity_ms as usize) / 1000;

    let adj_ms_u32 = lp.ring_ms;
```

- [ ] **Step 3: Use `lp.adaptive_min_ms` for the controller**

Find `let adaptive_min_ms = if is_loopback { ADAPTIVE_MIN_LOOPBACK_MS.max(min_buffer_ms) } else { ADAPTIVE_MIN_DEFAULT_MS.max(min_buffer_ms) };` and replace it with:

```rust
        let adaptive_min_ms = lp.adaptive_min_ms;
```

Then find the `AdaptiveBuffer::new(...)` call and change its `max` argument so it uses `lp.adaptive_max_ms`:

```rust
        let mut adaptive = self::buffer::AdaptiveBuffer::new(
            adaptive_min_ms,
            lp.adaptive_max_ms.max(adaptive_min_ms),
            super::constants::ADAPTIVE_BUFFER_STEP_MS,
            adj_ms_u32,
            6,
        );
```

- [ ] **Step 4: Use `effective_chunk` everywhere the network chunk is used**

`effective_chunk` and `lp` are computed in `run` (outside the network-thread closure) — move them above the `let bytes_sent_clone = ...` clones, and add `let effective_chunk_net = effective_chunk;` to the clone list so the closure can capture it. Then:
- In the closure, replace `let mut temp_buffer = vec![0.0f32; chunk_size as usize * device_channels_net as usize];` → use `effective_chunk_net`.
- Replace `let min_chunk_samples = chunk_size as usize * device_channels_net as usize;` → use `effective_chunk_net`.
- Outside the closure, replace `let capacity_hint = chunk_size as usize * device_channels as usize;` → `let capacity_hint = effective_chunk as usize * device_channels as usize;`.

> The hardware `buffer_size` (cpal `BufferSize`) is a different concept and stays as-is.

- [ ] **Step 5: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8`
Expected: compiles; all tests pass. (Unused-constant warnings from `constants.rs` are removed in Task 1.4.)

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/audio/commands.rs src-tauri/src/audio/manager.rs src-tauri/src/audio/engine/mod.rs
git commit -m "feat(engine): drive buffers from latency profile instead of fixed floors"
```

### Task 1.4: Remove the now-unused buffer-floor constants

**Files:** Modify `src-tauri/src/audio/constants.rs`

- [ ] **Step 1: Delete the dead constants**

In `src-tauri/src/audio/constants.rs`, remove `DEFAULT_MIN_BUFFER_MS`, `LOOPBACK_MIN_BUFFER_MS`, `ADAPTIVE_MIN_DEFAULT_MS`, `ADAPTIVE_MIN_LOOPBACK_MS`, and `PREFILL_FRACTION` (and the `test_buffer_constants_ordering` test's assertions that reference them — keep only assertions about constants that still exist; if the test becomes empty, delete it).

- [ ] **Step 2: Verify compile + clippy clean**

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings 2>&1 | tail -5 && cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -3`
Expected: clippy clean (no unused const); all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/constants.rs
git commit -m "refactor(audio): remove fixed buffer floors superseded by latency profiles"
```

### Task 1.5: Frontend latency profile

**Files:** Modify `src/stores/settings.ts`, `src/stores/stream.ts`, `src/components/tabs/AdvancedTab.vue`

- [ ] **Step 1: Replace `networkPreset` with `latencyProfile` in the store**

In `src/stores/settings.ts`:
- Replace `const networkPreset = ref("custom");` with `const latencyProfile = ref("balanced");`.
- Delete the `PRESETS` constant, the `PresetValue` interface, and the `applyPreset` function (the backend now derives buffers from the profile).
- In `SettingsDict`, replace `network_preset?: string;` with `latency_profile?: string;`.
- In `loadSettings`, replace `if (s.network_preset) networkPreset.value = s.network_preset as string;` with `if (s.latency_profile) latencyProfile.value = s.latency_profile as string;`.
- In `saveSettings`, replace `network_preset: networkPreset.value,` with `latency_profile: latencyProfile.value,`.
- In the store's `return { ... }`, replace `networkPreset,` with `latencyProfile,` and remove `applyPreset,`.

- [ ] **Step 2: Send `latencyProfile` to the backend**

In `src/stores/stream.ts`, in the `invoke("start_stream", { ... })` object inside `startStream`, add:
```ts
        latencyProfile: settings.latencyProfile,
```

- [ ] **Step 3: Replace the Network Presets UI with a Latency Profile selector**

In `src/components/tabs/AdvancedTab.vue`, replace the entire "Network Presets" `<section>` (the one with `id="network-preset"` and `@update:model-value="onPresetChange"`) with:

```vue
    <!-- Latency Profile -->
    <section class="glass-card">
      <h3 class="border-b border-white/10 pb-2 mb-4 text-sm font-semibold text-slate-300">
        Latency Profile
      </h3>
      <SelectField id="latency-profile" v-model="settings.latencyProfile" label="Latency vs Robustness">
        <option value="ultra-low">Ultra-low latency</option>
        <option value="balanced">Balanced</option>
        <option value="robust">Robust (high jitter tolerance)</option>
        <option value="custom">Custom (use Audio tab buffers)</option>
      </SelectField>
      <p class="mt-2 text-[11px] text-white/50">
        Lower latency uses smaller buffers (best on wired/quiet networks). Loopback capture needs more
        buffering — ultra-low may stutter.
      </p>
    </section>
```

Then in the `<script setup>` of `AdvancedTab.vue`, delete the `onPresetChange` function and the `names` map, and remove `useStreamStore`/`stream` if they become unused (run the linter to confirm).

- [ ] **Step 4: Verify frontend**

Run: `pnpm typecheck && pnpm lint && pnpm test 2>&1 | tail -5`
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add src/stores/settings.ts src/stores/stream.ts src/components/tabs/AdvancedTab.vue
git commit -m "feat(ui): configurable latency profile replacing network presets"
```

---

## Milestone 2 — IPv6 + hostnames

### Task 2.1: Pure `transport/resolve.rs` (TDD)

**Files:** Create `src-tauri/src/audio/transport/resolve.rs`; Modify `src-tauri/src/audio/transport/mod.rs`

- [ ] **Step 1: Declare the module**

In `src-tauri/src/audio/transport/mod.rs`, add to the `pub mod` block: `pub mod resolve;`.

- [ ] **Step 2: Write the module + tests**

Create `src-tauri/src/audio/transport/resolve.rs`:

```rust
//! Target address resolution: hostname / IPv4 / IPv6 literal → socket addresses.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs};

/// Resolves `host:port` to one or more socket addresses. `host` may be a
/// hostname, an IPv4 literal, or an IPv6 literal. Errors if nothing resolves.
pub fn resolve_target(host: &str, port: u16) -> io::Result<Vec<SocketAddr>> {
    let addrs: Vec<SocketAddr> = (host, port).to_socket_addrs()?.collect();
    if addrs.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no addresses resolved for {host}"),
        ));
    }
    Ok(addrs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn resolves_ipv4_literal() {
        let addrs = resolve_target("127.0.0.1", 4953).unwrap();
        assert!(addrs.iter().any(|a| a.ip() == "127.0.0.1".parse::<IpAddr>().unwrap()
            && a.port() == 4953));
    }

    #[test]
    fn resolves_ipv6_literal() {
        let addrs = resolve_target("::1", 4953).unwrap();
        assert!(addrs.iter().any(|a| a.ip() == "::1".parse::<IpAddr>().unwrap()));
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml transport::resolve 2>&1 | tail -8`
Expected: 2 tests pass (IP literals need no DNS).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/transport/mod.rs src-tauri/src/audio/transport/resolve.rs
git commit -m "feat(transport): tested host/IPv4/IPv6 address resolution"
```

### Task 2.2: Client connect via resolved addresses

**Files:** Modify `src-tauri/src/audio/transport/tcp_client.rs`

- [ ] **Step 1: Rewrite `connect` to resolve and try each address**

Replace the body of `connect` in `src-tauri/src/audio/transport/tcp_client.rs` with:

```rust
//! TCP client connection setup.

use socket2::{Domain, Protocol, Socket, TcpKeepalive, Type};
use std::io;
use std::net::TcpStream;
use std::time::Duration;

/// Connects to `host:port` (hostname, IPv4, or IPv6) with audio-friendly socket
/// options and the requested DSCP marking. Tries each resolved address until one
/// connects. Returns a ready-to-write blocking `TcpStream`.
pub fn connect(
    host: &str,
    port: u16,
    dscp_strategy: &str,
    connect_timeout: Duration,
) -> io::Result<TcpStream> {
    let addrs = super::resolve::resolve_target(host, port)?;
    let tos = u32::from(super::dscp::dscp_to_tos(dscp_strategy));

    let mut last_err: Option<io::Error> =
        None;
    for addr in addrs {
        let domain = if addr.is_ipv6() { Domain::IPV6 } else { Domain::IPV4 };
        let socket = match Socket::new(domain, Type::STREAM, Some(Protocol::TCP)) {
            Ok(s) => s,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };
        let _ = socket.set_send_buffer_size(32 * 1024);
        let _ = socket.set_nodelay(true);
        let keepalive = TcpKeepalive::new()
            .with_time(Duration::from_secs(10))
            .with_interval(Duration::from_secs(1));
        let _ = socket.set_tcp_keepalive(&keepalive);
        if tos > 0 {
            let _ = socket.set_tos(tos);
        }

        match socket.connect_timeout(&addr.into(), connect_timeout) {
            Ok(()) => return Ok(socket.into()),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        io::Error::new(io::ErrorKind::AddrNotAvailable, "no address could be connected")
    }))
}
```

> Note: `set_tos` only applies to IPv4 sockets; on IPv6 it is a best-effort no-op (already wrapped in `let _ =`). The call site in `engine::run` passes `&ip_clone` as `host` — it already does; no change needed there.

- [ ] **Step 2: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: compiles; all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/audio/transport/tcp_client.rs
git commit -m "feat(transport): client connects via hostnames and IPv6"
```

### Task 2.3: Dual-stack server bind

**Files:** Modify `src-tauri/src/audio/engine/mod.rs`

- [ ] **Step 1: Replace the IPv4-only listener bind**

In `engine::run`'s network thread, find the listener bind:
```rust
        let listener = if is_server_clone {
            match std::net::TcpListener::bind(format!("0.0.0.0:{}", port)) {
                Ok(l) => {
                    let _ = l.set_nonblocking(true);
                    emit_log(&app_handle_net, "success", format!("TCP Server listening on 0.0.0.0:{}", port));
                    Some(l)
                }
                Err(e) => {
                    emit_log(&app_handle_net, "error", format!("Failed to bind TCP server port: {}", e));
                    None
                }
            }
        } else {
            None
        };
```

Replace it with a dual-stack bind that falls back to IPv4:

```rust
        let listener = if is_server_clone {
            match bind_dual_stack(port) {
                Ok((l, label)) => {
                    let _ = l.set_nonblocking(true);
                    emit_log(&app_handle_net, "success", format!("TCP Server listening on {} (port {})", label, port));
                    Some(l)
                }
                Err(e) => {
                    emit_log(&app_handle_net, "error", format!("Failed to bind TCP server port: {}", e));
                    None
                }
            }
        } else {
            None
        };
```

- [ ] **Step 2: Add the `bind_dual_stack` helper**

Add this free function in `engine/mod.rs` (above `pub fn run`):

```rust
/// Binds a listener that accepts both IPv6 and IPv4 (via an IPv6 dual-stack
/// socket). Falls back to IPv4-only if the dual-stack bind fails (e.g. IPv6
/// disabled). Returns the listener and a label describing what it bound to.
fn bind_dual_stack(port: u16) -> std::io::Result<(std::net::TcpListener, &'static str)> {
    use socket2::{Domain, Protocol, Socket, Type};
    use std::net::{Ipv6Addr, SocketAddr};

    let try_v6 = || -> std::io::Result<std::net::TcpListener> {
        let socket = Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP))?;
        socket.set_only_v6(false)?; // accept mapped IPv4 too
        let _ = socket.set_reuse_address(true);
        let addr: SocketAddr = (Ipv6Addr::UNSPECIFIED, port).into();
        socket.bind(&addr.into())?;
        socket.listen(128)?;
        Ok(socket.into())
    };

    match try_v6() {
        Ok(l) => Ok((l, "[::] (dual-stack)")),
        Err(_) => {
            let l = std::net::TcpListener::bind(("0.0.0.0", port))?;
            Ok((l, "0.0.0.0 (IPv4 only)"))
        }
    }
}
```

- [ ] **Step 3: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: compiles; all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/engine/mod.rs
git commit -m "feat(transport): dual-stack server bind with IPv4 fallback"
```

---

## Milestone 3 — IP/CIDR allowlist

### Task 3.1: Pure `transport/allowlist.rs` (TDD)

**Files:** Create `src-tauri/src/audio/transport/allowlist.rs`; Modify `src-tauri/src/audio/transport/mod.rs`

- [ ] **Step 1: Declare the module**

In `src-tauri/src/audio/transport/mod.rs`, add `pub mod allowlist;`.

- [ ] **Step 2: Write the module + tests**

Create `src-tauri/src/audio/transport/allowlist.rs`:

```rust
//! Server connection allowlist (IP / CIDR matching), pure & testable.

use ipnet::IpNet;
use std::net::IpAddr;

/// Parses a list of IP or CIDR rules separated by commas, spaces, or newlines.
/// A bare IP (`10.0.0.5`, `fe80::1`) becomes a single-host network. Invalid
/// tokens are skipped.
pub fn parse_rules(s: &str) -> Vec<IpNet> {
    s.split([',', ' ', '\n', '\t', '\r'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .filter_map(|t| match t.parse::<IpNet>() {
            Ok(net) => Some(net),
            Err(_) => t.parse::<IpAddr>().ok().map(IpNet::from),
        })
        .collect()
}

/// Whether `peer` is allowed. Empty `rules` returns `allow_if_empty` (open by
/// default); otherwise true iff some rule contains `peer`.
pub fn is_allowed(peer: IpAddr, rules: &[IpNet], allow_if_empty: bool) -> bool {
    if rules.is_empty() {
        return allow_if_empty;
    }
    rules.iter().any(|net| net.contains(&peer))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn empty_rules_use_default() {
        assert!(is_allowed(ip("10.0.0.1"), &[], true));
        assert!(!is_allowed(ip("10.0.0.1"), &[], false));
    }

    #[test]
    fn exact_ipv4_match() {
        let rules = parse_rules("192.168.1.50, 10.0.0.5");
        assert!(is_allowed(ip("10.0.0.5"), &rules, false));
        assert!(!is_allowed(ip("10.0.0.6"), &rules, false));
    }

    #[test]
    fn cidr_v4_range() {
        let rules = parse_rules("192.168.1.0/24");
        assert!(is_allowed(ip("192.168.1.200"), &rules, false));
        assert!(!is_allowed(ip("192.168.2.1"), &rules, false));
    }

    #[test]
    fn cidr_v6_range() {
        let rules = parse_rules("fe80::/10");
        assert!(is_allowed(ip("fe80::abcd"), &rules, false));
        assert!(!is_allowed(ip("2001:db8::1"), &rules, false));
    }

    #[test]
    fn invalid_tokens_are_skipped() {
        let rules = parse_rules("not-an-ip, 10.0.0.5, , 999.999.0.0/8");
        assert_eq!(rules.len(), 1);
        assert!(is_allowed(ip("10.0.0.5"), &rules, false));
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml transport::allowlist 2>&1 | tail -8`
Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/transport/mod.rs src-tauri/src/audio/transport/allowlist.rs
git commit -m "feat(transport): tested IP/CIDR allowlist matching"
```

### Task 3.2: Thread `allowlist` and enforce it on accept

**Files:** Modify `commands.rs`, `manager.rs`, `engine/mod.rs`

- [ ] **Step 1: Thread the param** (same pattern as Task 1.2/1.3)

- In `commands.rs`: add `allowlist: String,` to `start_stream` (after `latency_profile: String,`) and `allowlist,` to the `AudioCommand::Start { ... }`.
- In `manager.rs`: add `allowlist: String,` to the `AudioCommand::Start` variant and its destructuring, and `allowlist,` to the `super::engine::run(...)` call (after `latency_profile,`).
- In `engine/mod.rs`: add `allowlist: String,` to `run`'s signature (after `latency_profile: String,`).

- [ ] **Step 2: Parse the rules once, before the network thread**

In `engine::run`, near the other `let *_clone = ...;` bindings (before `thread_builder.spawn`), add:
```rust
    let allow_rules = super::transport::allowlist::parse_rules(&allowlist);
```
This is `Vec<IpNet>` which is `Send`; capture it in the closure (it will move in automatically since it's used inside).

- [ ] **Step 3: Reject disallowed peers in the accept arm**

In the server accept arm, immediately after `Ok((mut stream, addr)) => {` and before `let _ = stream.set_nodelay(true);`, add:

```rust
                                if !super::transport::allowlist::is_allowed(addr.ip(), &allow_rules, true) {
                                    emit_log(&app_handle_net, "warning", format!("Rejected connection from {} (not in allowlist)", addr.ip()));
                                    let _ = stream.shutdown(std::net::Shutdown::Both);
                                    continue;
                                }
```

> `continue` returns to the top of the `while is_running_clone...` loop; the listener stays bound and keeps accepting.

- [ ] **Step 4: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: compiles; all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/audio/commands.rs src-tauri/src/audio/manager.rs src-tauri/src/audio/engine/mod.rs
git commit -m "feat(transport): enforce optional IP/CIDR allowlist on server accept"
```

### Task 3.3: Frontend allowlist field

**Files:** Modify `src/stores/settings.ts`, `src/stores/stream.ts`, `src/components/tabs/ConnectionTab.vue`

- [ ] **Step 1: Add `allowlist` to the store**

In `src/stores/settings.ts`:
- Add `const allowlist = ref("");` near the connection state (after `loopbackMode`).
- Add `allowlist?: string;` to `SettingsDict`.
- In `loadSettings`: `if (s.allowlist) allowlist.value = s.allowlist as string;`.
- In `saveSettings` settings object: `allowlist: allowlist.value,`.
- Add `allowlist,` to the store's `return { ... }`.

- [ ] **Step 2: Send it to the backend**

In `src/stores/stream.ts`, add to the `invoke("start_stream", { ... })` object:
```ts
        allowlist: settings.allowlist,
```

- [ ] **Step 3: Add the field to the server settings UI**

In `src/components/tabs/ConnectionTab.vue`, inside the destination/server `<section>`, add (only in server mode) an allowlist input below the port row:

```vue
      <InputField
        v-if="settings.isServer"
        id="allowlist-input"
        v-model="settings.allowlist"
        label="Allowlist (optional, IP/CIDR — empty = allow all)"
        placeholder="192.168.1.0/24, 10.0.0.5"
        :disabled="stream.isStreaming"
        class="mt-3"
      />
```

- [ ] **Step 4: Verify frontend**

Run: `pnpm typecheck && pnpm lint && pnpm test 2>&1 | tail -5`
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add src/stores/settings.ts src/stores/stream.ts src/components/tabs/ConnectionTab.vue
git commit -m "feat(ui): optional IP/CIDR allowlist field for server mode"
```

---

## Milestone 4 — Genericize terminology

### Task 4.1: Backend + frontend strings

**Files:** Modify `engine/mod.rs`, `src/stores/stream.ts`, `src/components/tabs/ConnectionTab.vue`, `src/components/tabs/AudioTab.vue`

- [ ] **Step 1: Find every vendor reference**

Run: `git grep -in "snap" -- ':!docs' ':!CHANGELOG.md' | grep -vi snapshot`
Expected: a list of the remaining vendor-name occurrences to fix (comments, labels, the generated config string).

- [ ] **Step 2: Backend comment**

In `engine/mod.rs`, the resampling comment that names a specific server (≈ "breaks protocol synchronization (e.g. with Snapserver)") → reword generically: "breaks protocol synchronization with the downstream audio receiver."

- [ ] **Step 3: Generic server connection info in the store**

In `src/stores/stream.ts`, the `startStream` server branch sets `tcpUrl`, `httpUrl`, and a vendor-specific config string. Replace the vendor config with a generic hint. Find:
```ts
        snapcastConfig.value = `[stream]\nsource = tcp://${lip}:${p}?name=TCPStreamer&mode=client`;
```
Replace with:
```ts
        snapcastConfig.value = `Use this as a TCP source in your audio receiver:\ntcp://${lip}:${p}`;
```
(Keep the `snapcastConfig` ref name OR rename to `receiverHint` — if renaming, update the ref declaration, the `return`, and `AudioTab.vue`. Minimal path: keep the ref name, change only the string.)

- [ ] **Step 4: Generic labels**

- `src/components/tabs/ConnectionTab.vue`: change the mode help text ("Client Mode: Connects to a Snapserver." / "Server Mode: Waits for Snapservers to connect.") to "Client: connect to an audio receiver." / "Server: wait for an audio receiver to connect." Change the option label "Server (Listen for Connections)" stays; "Client (Send Audio to IP)" stays.
- `src/components/tabs/AudioTab.vue`: the server URLs card labels ("Snapcast Server Address", "Snapserver Config (snapserver.conf)") → "Receiver Address (TCP)" and "Connection Info".

- [ ] **Step 5: Verify frontend + backend**

Run: `pnpm typecheck && pnpm lint && pnpm test && cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -3`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: genericize terminology (audio receiver, not a specific vendor)"
```

### Task 4.2: Docs

**Files:** Modify `README.md`, `CONTRIBUTING.md`

- [ ] **Step 1: Replace vendor names in docs**

In `README.md` and `CONTRIBUTING.md`, replace vendor-specific phrasing with generic terms: "audio receivers such as Sonium", "a TCP audio source", "the receiver". Keep technically accurate details (PCM/WAV, ports as generic examples). Run `git grep -in "snap" -- README.md CONTRIBUTING.md` and fix each hit.

- [ ] **Step 2: Verify formatting**

Run: `pnpm format:check 2>&1 | tail -3`
Expected: green (run `pnpm format` if prettier reformats the docs, then re-check).

- [ ] **Step 3: Commit**

```bash
git add README.md CONTRIBUTING.md
git commit -m "docs: genericize terminology"
```

---

## Milestone 5 — Verification

### Task 5.1: Full green gate + behavioural check

- [ ] **Step 1: Complete check suite**

Run:
```bash
pnpm test && pnpm typecheck && pnpm lint && pnpm format:check && \
cargo test --manifest-path src-tauri/Cargo.toml && \
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```
Expected: all green; Rust tests up by ~12 (latency 5, resolve 2, allowlist 5).

- [ ] **Step 2: Confirm no vendor name remains in shipped surfaces**

Run: `git grep -in "snap" -- src src-tauri/src README.md CONTRIBUTING.md | grep -vi snapshot`
Expected: no functional references (empty, or only incidental words).

- [ ] **Step 3: Behavioural verification**

Run `pnpm tauri dev` and confirm:
1. **Latency profile:** switch Ultra-low vs Robust; the "Ring buffer: …ms" log differs greatly (ultra-low ~100ms, robust ~3000ms); no forced 5s/8s floor.
2. **Hostname/IPv6:** in client mode, connect to a receiver by hostname and by an IPv6 literal — both connect.
3. **Dual-stack:** server logs "[::] (dual-stack)" (or the IPv4 fallback with its reason).
4. **Allowlist:** set `allowlist` to a CIDR that excludes the client → connection rejected in logs; clear it → accepted.

- [ ] **Step 4: Final commit (if needed)**

```bash
git add -A && git commit -m "chore: Phase 2A verification fixes"
```

---

## Self-Review (author)

- **Spec coverage:** §5.1 latency profile → M1 (latency.rs + engine wiring + constants removal + frontend); §5.2 IPv6/hostnames → M2 (resolve.rs + client + dual-stack bind); §5.3 allowlist → M3 (allowlist.rs + accept check + frontend); §5.4 genericize → M4; tests (§6) across M1–M3.
- **Type consistency:** `latency::params(&str,bool)->LatencyParams{ring_ms,adaptive_min_ms,adaptive_max_ms,chunk_size,prefill_ms}`; `resolve::resolve_target(&str,u16)->io::Result<Vec<SocketAddr>>`; `allowlist::{parse_rules(&str)->Vec<IpNet>, is_allowed(IpAddr,&[IpNet],bool)->bool}`. New params `latency_profile: String`, `allowlist: String` are added in the same order (after `max_buffer_ms`) across `start_stream` → `AudioCommand::Start` → `engine::run`. Frontend sends camelCase `latencyProfile`/`allowlist` (Tauri maps to snake_case).
- **No placeholders:** every code step shows complete code; the genericization steps name exact strings + a grep to catch the rest (a deterministic sweep, not a vague "handle the rest").

## Execution Handoff

(see below — choose subagent-driven or inline)
