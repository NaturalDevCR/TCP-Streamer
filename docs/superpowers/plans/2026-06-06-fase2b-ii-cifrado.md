# Phase 2B-ii: Native UDP Encryption (AEAD/PSK) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add optional, per-packet authenticated encryption to the native UDP transport: a PSK-derived key (HKDF-SHA256) and ChaCha20-Poly1305 over each audio frame, with replay protection. Off by default (empty PSK); when both peers share a PSK, the audio is encrypted+authenticated and forged/replayed packets are rejected.

**Architecture:** A pure, tested `transport/udp/crypto.rs` (key derivation, seal/open, sliding replay window). The source seals each frame's payload (AAD = the plaintext audio header, nonce = session-salt ‖ seq) and sets the `encrypted` flag bit; the sink opens it and runs the replay window. The PSK and per-session salts ride the existing SUBSCRIBE (`salt_b`) / STREAM_INFO (`salt_a`) handshake fields, which 2B-i already reserved.

**Tech Stack:** Rust; `chacha20poly1305` + `hkdf` + `sha2` (RustCrypto). **API note:** the chacha20poly1305/hkdf crates are not cached locally — the code uses the stable RustCrypto AEAD/HKDF API; if a method name differs in the resolved version, adjust to that version's `aead`/`hkdf` API (the algorithm and structure are exact).

**Spec:** `docs/superpowers/specs/2026-06-06-fase2b-modo-nativo-design.md` §5.4.
**Depends on:** 2B-i committed (`transport/udp/{packet,source,sink}`, handshake with `salt_a`/`salt_b`). **Anchor edits by quoted code.**

---

## File Structure

| File                                                         | Responsibility                                    | Status |
| ------------------------------------------------------------ | ------------------------------------------------- | ------ |
| `src-tauri/Cargo.toml`                                       | add `chacha20poly1305`, `hkdf`, `sha2`            | Modify |
| `src-tauri/src/audio/transport/udp/crypto.rs`                | **(pure)** derive_key, seal/open, replay window   | Create |
| `src-tauri/src/audio/transport/udp/mod.rs`                   | `pub mod crypto;`                                 | Modify |
| `src-tauri/src/audio/transport/udp/source.rs`                | seal payload + set encrypted flag + random salt_a | Modify |
| `src-tauri/src/audio/transport/udp/sink.rs`                  | open payload + replay window                      | Modify |
| `src-tauri/src/audio/engine/{mod,sink}.rs`                   | thread `psk` to source/sink                       | Modify |
| `src-tauri/src/audio/commands.rs` · `manager.rs`             | thread `psk` (start_stream native + start_sink)   | Modify |
| `src/stores/settings.ts` · `stream.ts` · `ConnectionTab.vue` | PSK field                                         | Modify |

---

## Milestone 0 — Dependencies

### Task 0.1: Add the crypto crates

**Files:** Modify `src-tauri/Cargo.toml`

- [ ] **Step 1: Add dependencies**

In `src-tauri/Cargo.toml` under `[dependencies]`, add:

```toml
chacha20poly1305 = "0.10"
hkdf = "0.12"
sha2 = "0.10"
```

- [ ] **Step 2: Verify they resolve**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: downloads + builds.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "build: add chacha20poly1305/hkdf/sha2 for native UDP encryption"
```

---

## Milestone 1 — `transport/udp/crypto.rs` (pure, TDD)

### Task 1.1: Key derivation, seal/open, replay window

**Files:** Create `src-tauri/src/audio/transport/udp/crypto.rs`; Modify `src-tauri/src/audio/transport/udp/mod.rs`

- [ ] **Step 1: Declare the module**

In `transport/udp/mod.rs`, add `pub mod crypto;`.

- [ ] **Step 2: Write the module + tests**

Create `src-tauri/src/audio/transport/udp/crypto.rs`:

```rust
//! Per-packet AEAD for the native UDP transport (pure, testable).
//!
//! Key = HKDF-SHA256(PSK, salt_a ‖ salt_b). Each audio frame is sealed with
//! ChaCha20-Poly1305; the nonce is `session_salt(4) ‖ seq(8)` and the plaintext
//! audio header is the AEAD associated data (authenticated, not encrypted). A
//! sliding replay window rejects duplicated/old sequence numbers.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use sha2::Sha256;

/// Derives the 32-byte session key from the PSK and both peers' salts.
pub fn derive_key(psk: &str, salt_a: u64, salt_b: u64) -> [u8; 32] {
    let mut salt = [0u8; 16];
    salt[..8].copy_from_slice(&salt_a.to_be_bytes());
    salt[8..].copy_from_slice(&salt_b.to_be_bytes());
    let hk = Hkdf::<Sha256>::new(Some(&salt), psk.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(b"tcp-streamer/udp/v1", &mut okm)
        .expect("32 is a valid HKDF length");
    okm
}

/// The 4-byte nonce salt both peers compute identically from the session salts.
pub fn nonce_salt(salt_a: u64, salt_b: u64) -> u32 {
    ((salt_a ^ salt_b) & 0xFFFF_FFFF) as u32
}

fn nonce(salt: u32, seq: u64) -> [u8; 12] {
    let mut n = [0u8; 12];
    n[..4].copy_from_slice(&salt.to_be_bytes());
    n[4..].copy_from_slice(&seq.to_be_bytes());
    n
}

/// Seals `plaintext` (returns ciphertext+tag). `aad` is authenticated, not
/// encrypted (use the audio header bytes).
pub fn seal(key: &[u8; 32], salt: u32, seq: u64, aad: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .encrypt(Nonce::from_slice(&nonce(salt, seq)), Payload { msg: plaintext, aad })
        .expect("encryption is infallible for valid inputs")
}

/// Opens `ciphertext`; returns `None` if authentication fails (tampered/forged).
pub fn open(key: &[u8; 32], salt: u32, seq: u64, aad: &[u8], ciphertext: &[u8]) -> Option<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .decrypt(Nonce::from_slice(&nonce(salt, seq)), Payload { msg: ciphertext, aad })
        .ok()
}

/// Sliding window (64 seqs) rejecting replayed or too-old sequence numbers.
pub struct ReplayWindow {
    highest: u64,
    bitmap: u64,
    started: bool,
}

impl Default for ReplayWindow {
    fn default() -> Self {
        Self { highest: 0, bitmap: 0, started: false }
    }
}

impl ReplayWindow {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if `seq` is fresh (and records it); false if replayed/old.
    pub fn check_and_update(&mut self, seq: u64) -> bool {
        if !self.started {
            self.started = true;
            self.highest = seq;
            self.bitmap = 1;
            return true;
        }
        if seq > self.highest {
            let shift = seq - self.highest;
            self.bitmap = if shift >= 64 { 0 } else { self.bitmap << shift };
            self.bitmap |= 1;
            self.highest = seq;
            true
        } else {
            let diff = self.highest - seq;
            if diff >= 64 {
                return false; // too old
            }
            let mask = 1u64 << diff;
            if self.bitmap & mask != 0 {
                false // already seen
            } else {
                self.bitmap |= mask;
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_derivation_is_deterministic_and_salt_sensitive() {
        assert_eq!(derive_key("pw", 1, 2), derive_key("pw", 1, 2));
        assert_ne!(derive_key("pw", 1, 2), derive_key("pw", 9, 2));
        assert_ne!(derive_key("pw", 1, 2), derive_key("other", 1, 2));
    }

    #[test]
    fn seal_then_open_roundtrips() {
        let k = derive_key("secret", 11, 22);
        let salt = nonce_salt(11, 22);
        let aad = b"header-bytes";
        let ct = seal(&k, salt, 5, aad, b"hello pcm");
        assert_eq!(open(&k, salt, 5, aad, &ct).as_deref(), Some(&b"hello pcm"[..]));
    }

    #[test]
    fn open_rejects_tampered_ciphertext_aad_key_or_seq() {
        let k = derive_key("secret", 11, 22);
        let salt = nonce_salt(11, 22);
        let mut ct = seal(&k, salt, 5, b"hdr", b"pcm");
        let good = ct.clone();
        ct[0] ^= 0xFF;
        assert!(open(&k, salt, 5, b"hdr", &ct).is_none()); // tampered ct
        assert!(open(&k, salt, 5, b"OTHER", &good).is_none()); // wrong aad
        assert!(open(&k, salt, 6, b"hdr", &good).is_none()); // wrong seq (nonce)
        let k2 = derive_key("wrong", 11, 22);
        assert!(open(&k2, salt, 5, b"hdr", &good).is_none()); // wrong key
    }

    #[test]
    fn replay_window_accepts_new_rejects_dup_and_old() {
        let mut w = ReplayWindow::new();
        assert!(w.check_and_update(10)); // first
        assert!(w.check_and_update(11));
        assert!(!w.check_and_update(11)); // dup
        assert!(w.check_and_update(12));
        assert!(w.check_and_update(9)); // within window, not yet seen
        assert!(!w.check_and_update(9)); // now a dup
        assert!(!w.check_and_update(12 - 64)); // too old
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml transport::udp::crypto 2>&1 | tail -10`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/transport/udp/mod.rs src-tauri/src/audio/transport/udp/crypto.rs
git commit -m "feat(udp): tested AEAD/PSK crypto (HKDF + ChaCha20-Poly1305 + replay window)"
```

---

## Milestone 2 — Encrypt the source, decrypt the sink

### Task 2.1: Source seals frames when a PSK is set

**Files:** Modify `src-tauri/src/audio/transport/udp/source.rs`

> 2B-i's `UdpSource` holds `info: StreamInfo` (with `salt_a`), `seq`, and `send_audio(payload)` which calls `packet::encode_audio(&AudioHeader{flags,seq,ts_us}, payload, &mut self.out)`. Anchor to that code.

- [ ] **Step 1: Give `UdpSource` an optional key + random salt_a**

Add fields and a setter. In `UdpSource::bind`, set `info.salt_a` to a random value (e.g. derived from `std::time::SystemTime` nanos) instead of 0. Add:

```rust
    key: Option<[u8; 32]>,
    nonce_salt: u32,
    psk: String,
```

Initialize `key: None, nonce_salt: 0, psk` (pass `psk: String` into `bind`). Set `info.salt_a` to a random `u64`.

- [ ] **Step 2: Derive the key when a subscriber arrives**

In `poll_subscribe`, after `packet::decode_subscribe(&buf[..n])` yields `salt_b` and before/after sending STREAM_INFO: if `!self.psk.is_empty()`, set

```rust
                let key = super::crypto::derive_key(&self.psk, self.info.salt_a, salt_b);
                self.nonce_salt = super::crypto::nonce_salt(self.info.salt_a, salt_b);
                self.key = Some(key);
                self.info.flags = 1; // encrypted
```

(Set `self.info.flags = 0` and `self.key = None` when `psk` is empty — the default.) `decode_subscribe` currently returns `Option<u64>` (the salt_b) — use it.

- [ ] **Step 3: Seal in `send_audio`**

In `send_audio`, build the header, then if `self.key` is `Some`, seal the payload with AAD = the encoded header bytes. Concretely, encode the header first into a small buffer, seal, then append:

```rust
        let h = AudioHeader { flags: if self.key.is_some() { 1 } else { 0 }, seq: self.seq, ts_us };
        if let Some(key) = &self.key {
            // header bytes (without payload) serve as AAD
            let mut hdr = Vec::new();
            packet::encode_audio(&h, &[], &mut hdr); // header-only, AUDIO_HEADER_LEN bytes
            let ct = super::crypto::seal(key, self.nonce_salt, self.seq, &hdr[..packet::AUDIO_HEADER_LEN], payload);
            packet::encode_audio(&h, &ct, &mut self.out);
        } else {
            packet::encode_audio(&h, payload, &mut self.out);
        }
        let _ = self.socket.send_to(&self.out, peer);
        self.seq = self.seq.wrapping_add(1);
```

- [ ] **Step 4: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: compiles; all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/audio/transport/udp/source.rs
git commit -m "feat(udp): source seals frames with AEAD when PSK is set"
```

### Task 2.2: Sink opens frames + replay window

**Files:** Modify `src-tauri/src/audio/transport/udp/sink.rs`

> 2B-i's `receive_loop(socket, salt_b, lost_after, producer, running)` decodes each datagram via `packet::decode_audio` and pushes the payload into the jitter buffer. The STREAM_INFO (with `salt_a` + `flags`) was obtained by `subscribe`.

- [ ] **Step 1: Pass the key material into the receive loop**

Change `receive_loop` to also take `key: Option<[u8;32]>` and `nonce_salt: u32` (derived by the caller from PSK + salts after the handshake). Add a `let mut replay = super::crypto::ReplayWindow::new();` at the top.

- [ ] **Step 2: Open + replay-check each audio frame**

Replace the body that does `if let Some((h, payload)) = packet::decode_audio(&buf[..n]) { jb.push(h.seq, payload.to_vec()); }` with:

```rust
                if let Some((h, payload)) = packet::decode_audio(&buf[..n]) {
                    let pcm = if h.flags & 1 != 0 {
                        match &key {
                            Some(k) => {
                                if !replay.check_and_update(h.seq) {
                                    continue; // replayed/old
                                }
                                let mut hdr = Vec::new();
                                super::packet::encode_audio(
                                    &super::packet::AudioHeader { flags: h.flags, seq: h.seq, ts_us: h.ts_us },
                                    &[],
                                    &mut hdr,
                                );
                                match super::crypto::open(k, nonce_salt, h.seq, &hdr[..super::packet::AUDIO_HEADER_LEN], payload) {
                                    Some(pt) => pt,
                                    None => continue, // forged/tampered
                                }
                            }
                            None => continue, // encrypted frame but no key configured
                        }
                    } else {
                        payload.to_vec()
                    };
                    jb.push(h.seq, pcm);
                }
```

- [ ] **Step 3: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: compiles; all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/transport/udp/sink.rs
git commit -m "feat(udp): sink opens AEAD frames + replay protection"
```

---

## Milestone 3 — Thread the PSK through

### Task 3.1: Backend plumbing

**Files:** Modify `engine/mod.rs`, `engine/sink.rs`, `manager.rs`, `commands.rs`

- [ ] **Step 1: Source side**

Thread a `psk: String` param: `start_stream` → `AudioCommand::Start` → `engine::run` (after `transport`). In `engine::run`'s UDP branch, pass `psk.clone()` into `UdpSource::bind(port, sample_rate_clone, device_channels_net, psk.clone())`.

- [ ] **Step 2: Sink side**

Thread `psk: String` into `start_sink` → `AudioCommand::StartSink` → `engine::sink::run_sink`. In `run_sink`, after `subscribe` returns `info` (with `info.salt_a` and `info.flags`), compute the key when a PSK is set:

```rust
    let (key, nonce_salt) = if !psk.is_empty() {
        (
            Some(super::super::transport::udp::crypto::derive_key(&psk, info.salt_a, salt_b)),
            super::super::transport::udp::crypto::nonce_salt(info.salt_a, salt_b),
        )
    } else {
        (None, 0)
    };
```

Pass `key, nonce_salt` into the `receive_loop(&socket, salt_b, lost_after, key, nonce_salt, prod, running_net)` call.

- [ ] **Step 3: Verify compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: compiles; all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/engine/mod.rs src-tauri/src/audio/engine/sink.rs src-tauri/src/audio/manager.rs src-tauri/src/audio/commands.rs
git commit -m "feat(audio): thread PSK to native source and sink"
```

### Task 3.2: Frontend PSK field

**Files:** Modify `src/stores/settings.ts`, `src/stores/stream.ts`, `src/components/tabs/ConnectionTab.vue`

- [ ] **Step 1: Store**

In `settings.ts`: add `const psk = ref("");`, add `psk?: string;` to `SettingsDict`, load (`if (s.psk) psk.value = s.psk as string;`) and save (`psk: psk.value,`), and export `psk`.

- [ ] **Step 2: Send it**

In `stream.ts`, add `psk: settings.psk,` to BOTH the sink `invoke("start_sink", {...})` and the source-native `invoke("start_stream", {...})` objects.

- [ ] **Step 3: UI field**

In `ConnectionTab.vue`, when the native UDP transport (source) or sink role is active, show a password `InputField` bound to `settings.psk`, label "Encryption key (PSK — empty = no encryption)", `type="password"`.

- [ ] **Step 4: Verify**

Run: `pnpm typecheck && pnpm lint && pnpm test && pnpm format:check 2>&1 | tail -4`
Expected: green.

- [ ] **Step 5: Commit**

```bash
git add src/stores/settings.ts src/stores/stream.ts src/components/tabs/ConnectionTab.vue
git commit -m "feat(ui): optional PSK field for native-mode encryption"
```

---

## Milestone 4 — Verification

### Task 4.1: Encrypted loopback test

**Files:** Modify `src-tauri/src/audio/transport/udp/tests_loopback.rs`

- [ ] **Step 1: Add an encrypted end-to-end assertion**

Extend the loopback test (or add a sibling) that derives a key on both sides from the same PSK + salts, has the source seal a frame and the sink open it, and asserts a frame sealed with a _different_ PSK fails to open. Use `crypto::{derive_key, nonce_salt, seal, open}` directly with the captured `salt_a`/`salt_b`:

```rust
#[test]
fn encrypted_frame_roundtrips_and_wrong_psk_fails() {
    use super::crypto::{derive_key, nonce_salt, open, seal};
    let (sa, sb) = (0x1111_2222_3333_4444u64, 0xAAAA_BBBB_CCCC_DDDDu64);
    let k = derive_key("hunter2", sa, sb);
    let ns = nonce_salt(sa, sb);
    let aad = b"audio-header";
    let ct = seal(&k, ns, 7, aad, b"pcm-frame");
    assert_eq!(open(&k, ns, 7, aad, &ct).as_deref(), Some(&b"pcm-frame"[..]));
    let kbad = derive_key("wrong", sa, sb);
    assert!(open(&kbad, ns, 7, aad, &ct).is_none());
}
```

- [ ] **Step 2: Full gate**

Run:

```bash
pnpm test && pnpm typecheck && pnpm lint && pnpm format:check && \
cargo test --manifest-path src-tauri/Cargo.toml && \
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: all green; Rust tests up by ~5 (crypto 4 + encrypted loopback 1).

- [ ] **Step 3: Manual check (two instances)**

Source (native UDP) + Sink, same PSK → audio plays. Change the sink's PSK → audio stops (frames rejected), logs show no playback. Empty PSK on both → still works (plaintext).

- [ ] **Step 4: Final commit (if needed)**

```bash
git add -A && git commit -m "chore: Phase 2B-ii verification fixes"
```

---

## Self-Review (author)

- **Spec coverage:** §5.4 (AEAD/PSK, HKDF, per-packet ChaCha20-Poly1305, AAD=header, nonce=salt‖seq, replay window, optional) → M1 (crypto.rs) + M2 (seal/open wiring) + M3 (PSK plumbing). Handshake salt fields (§5.5) already exist from 2B-i.
- **Type consistency:** `derive_key(&str,u64,u64)->[u8;32]`, `nonce_salt(u64,u64)->u32`, `seal(&[u8;32],u32,u64,&[u8],&[u8])->Vec<u8>`, `open(...)->Option<Vec<u8>>`, `ReplayWindow::{new,check_and_update(u64)->bool}`. Source builds AAD as the `AUDIO_HEADER_LEN` header bytes; sink reconstructs the identical header bytes for AAD. `psk: String` threaded in the same order on both source (`start_stream`) and sink (`start_sink`).
- **No placeholders:** complete code throughout; the only caveat is the RustCrypto API-version note in the header (algorithm fully specified).

## Next plan

- **2B-iii — Discovery (mDNS) + clock-drift:** `transport/discovery.rs` (advertise on source, browse on sink, `mdns-sd 0.11`) + `list_sources` command + UI picker; plus a tested clock-drift helper in the jitter path (drop/duplicate a mini-chunk on sustained drift).
