//! End-to-end encryption for feiq++ <-> feiq++ communication.
//! ECDH (x25519) key exchange + AES-256-GCM encryption via ring 0.17.
//! Only activated between feiq++ peers (detected via version string).

use ring::aead::{Aad, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey, AES_256_GCM};
use ring::agreement::{agree_ephemeral, EphemeralPrivateKey, UnparsedPublicKey, X25519};
use ring::rand::SystemRandom;

const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
const KEY_LEN: usize = 32;

/// Counter-based nonce sequence for AES-GCM
struct CounterNonceSequence([u8; NONCE_LEN]);

impl CounterNonceSequence {
    fn new() -> Self { Self([0u8; NONCE_LEN]) }
}

impl NonceSequence for CounterNonceSequence {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        let nonce = Nonce::assume_unique_for_key(self.0);
        for b in self.0.iter_mut().rev() {
            *b = b.wrapping_add(1);
            if *b != 0 { break; }
        }
        Ok(nonce)
    }
}

/// Encryptor state: raw key bytes + nonce sequence
pub struct FeiqEncryptor {
    key_bytes: [u8; KEY_LEN],
    nonce_seq: CounterNonceSequence,
}

/// Decryptor state: raw key bytes + nonce sequence
pub struct FeiqDecryptor {
    key_bytes: [u8; KEY_LEN],
    nonce_seq: CounterNonceSequence,
}

/// Generate ephemeral x25519 keypair
pub fn generate_keypair() -> Result<(EphemeralPrivateKey, Vec<u8>), ring::error::Unspecified> {
    let rng = SystemRandom::new();
    let private_key = EphemeralPrivateKey::generate(&X25519, &rng)?;
    let public_key = private_key.compute_public_key()?;
    Ok((private_key, public_key.as_ref().to_vec()))
}

/// Compute shared secret via ECDH
pub fn compute_shared_secret(
    private_key: EphemeralPrivateKey,
    peer_public: &[u8],
) -> Result<Vec<u8>, ring::error::Unspecified> {
    let peer_key = UnparsedPublicKey::new(&X25519, peer_public);
    agree_ephemeral(private_key, &peer_key, |key_material| {
        key_material.to_vec()
    })
}

/// Derive 32-byte key from shared secret
fn derive_key_bytes(shared_secret: &[u8]) -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    let len = std::cmp::min(KEY_LEN, shared_secret.len());
    key[..len].copy_from_slice(&shared_secret[..len]);
    key
}

/// Create encryptor from shared secret
pub fn create_encryptor(shared_secret: &[u8]) -> FeiqEncryptor {
    FeiqEncryptor {
        key_bytes: derive_key_bytes(shared_secret),
        nonce_seq: CounterNonceSequence::new(),
    }
}

/// Create decryptor from shared secret
pub fn create_decryptor(shared_secret: &[u8]) -> FeiqDecryptor {
    FeiqDecryptor {
        key_bytes: derive_key_bytes(shared_secret),
        nonce_seq: CounterNonceSequence::new(),
    }
}

/// Encrypt plaintext with AES-256-GCM
pub fn encrypt(plaintext: &[u8], enc: &mut FeiqEncryptor) -> Result<Vec<u8>, ring::error::Unspecified> {
    let nonce_bytes = enc.nonce_seq.0;
    enc.nonce_seq.advance()?; // advance for next call
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let unbound = UnboundKey::new(&AES_256_GCM, &enc.key_bytes)?;

    // Use LessSafeKey which takes individual nonces
    let key = ring::aead::LessSafeKey::new(unbound);
    let mut in_out = plaintext.to_vec();
    // seal_in_place_append_tag extends in_out with tag bytes internally
    key.seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)?;
    Ok(in_out)
}

/// Decrypt ciphertext with AES-256-GCM
pub fn decrypt(ciphertext: &[u8], dec: &mut FeiqDecryptor) -> Result<Vec<u8>, ring::error::Unspecified> {
    let nonce_bytes = dec.nonce_seq.0;
    dec.nonce_seq.advance()?;
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let unbound = UnboundKey::new(&AES_256_GCM, &dec.key_bytes)?;

    let key = ring::aead::LessSafeKey::new(unbound);
    let mut in_out = ciphertext.to_vec();
    let plaintext = key.open_in_place(nonce, Aad::empty(), &mut in_out)?;
    Ok(plaintext.to_vec())
}

/// Detect feiq++ peer
pub fn is_feiq_plus_plus(version: &str) -> bool {
    version.starts_with("feiq_plus_plus")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ecdh() {
        let (a_priv, a_pub) = generate_keypair().unwrap();
        let (b_priv, b_pub) = generate_keypair().unwrap();
        let a_secret = compute_shared_secret(a_priv, &b_pub).unwrap();
        let b_secret = compute_shared_secret(b_priv, &a_pub).unwrap();
        assert_eq!(a_secret, b_secret);
    }

    #[test]
    fn test_encrypt_roundtrip() {
        let (a_priv, a_pub) = generate_keypair().unwrap();
        let (b_priv, _) = generate_keypair().unwrap();
        let secret = compute_shared_secret(a_priv, &b_priv.compute_public_key().unwrap().as_ref()).unwrap();
        let secret2 = compute_shared_secret(b_priv, &a_pub).unwrap();

        let mut enc = create_encryptor(&secret);
        let mut dec = create_decryptor(&secret2);

        let msg = b"encrypted feiq++ message test";
        let ct = encrypt(msg, &mut enc).unwrap();
        assert_eq!(ct.len(), msg.len() + TAG_LEN); // message + 16-byte tag
        let pt = decrypt(&ct, &mut dec).unwrap();
        assert_eq!(&pt, msg);
    }

    #[test]
    fn test_detect() {
        assert!(is_feiq_plus_plus("feiq_plus_plus#128#MAC#0#0#0#1#9"));
        assert!(!is_feiq_plus_plus("1_lbt6_0#128#MAC#0#0#0#4001#9"));
    }
}
