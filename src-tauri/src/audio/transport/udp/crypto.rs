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
        .encrypt(
            Nonce::from_slice(&nonce(salt, seq)),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .expect("encryption is infallible for valid inputs")
}

/// Opens `ciphertext`; returns `None` if authentication fails (tampered/forged).
pub fn open(key: &[u8; 32], salt: u32, seq: u64, aad: &[u8], ciphertext: &[u8]) -> Option<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .decrypt(
            Nonce::from_slice(&nonce(salt, seq)),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .ok()
}

/// Sliding window (64 seqs) rejecting replayed or too-old sequence numbers.
#[derive(Default)]
pub struct ReplayWindow {
    highest: u64,
    bitmap: u64,
    started: bool,
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
        assert_eq!(
            open(&k, salt, 5, aad, &ct).as_deref(),
            Some(&b"hello pcm"[..])
        );
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
        assert!(w.check_and_update(100)); // first
        assert!(w.check_and_update(101));
        assert!(!w.check_and_update(101)); // dup
        assert!(w.check_and_update(102));
        assert!(w.check_and_update(99)); // within window, not yet seen
        assert!(!w.check_and_update(99)); // now a dup
        assert!(!w.check_and_update(35)); // too old (102 - 35 = 67 >= 64)
    }
}
