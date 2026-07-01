//! End-to-end encryption for feiq++ <-> feiq++ communication.
//! ECDH (x25519) key exchange + AES-256-GCM encryption via ring 0.17.
//! Only activated between feiq++ peers (detected via version string).

use ring::aead::{Aad, Nonce, NonceSequence, UnboundKey, AES_256_GCM};
use ring::agreement::{agree_ephemeral, EphemeralPrivateKey, UnparsedPublicKey, X25519};
use ring::hkdf;
use ring::rand::SecureRandom;
use ring::rand::SystemRandom;
use x25519_dalek::x25519;
use x25519_dalek::X25519_BASEPOINT_BYTES;

const NONCE_LEN: usize = 12;
#[allow(dead_code)]
const TAG_LEN: usize = 16;
const KEY_LEN: usize = 32;

/// Counter-based nonce sequence for AES-GCM.
///
/// Initialized with a random prefix to prevent nonce reuse when the same
/// ECDH shared secret is re-derived across reconnections (peer leaves and
/// re-joins). Without this, a new session with the same shared secret would
/// restart the counter at zero, reusing nonces and breaking AES-GCM security.
struct CounterNonceSequence([u8; NONCE_LEN]);

impl CounterNonceSequence {
    fn new() -> Self {
        let rng = SystemRandom::new();
        let mut nonce = [0u8; NONCE_LEN];
        rng.fill(&mut nonce[..4]).expect("OS RNG should not fail during nonce initialization");
        Self(nonce)
    }
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

/// Decryptor state: raw key bytes only (nonce is received as part of ciphertext)
pub struct FeiqDecryptor {
    key_bytes: [u8; KEY_LEN],
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

/// Derive 32-byte key from shared secret using HKDF-SHA256
fn derive_key_bytes(shared_secret: &[u8]) -> [u8; KEY_LEN] {
    let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, b"feiq-plus-plus-encryption-v1");
    let prk = salt.extract(shared_secret);
    let okm = prk
        .expand(&[b"AES-256-GCM-key"], hkdf::HKDF_SHA256)
        .expect("HKDF expand should not fail");
    let mut key = [0u8; KEY_LEN];
    okm.fill(&mut key).expect("HKDF fill should not fail");
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
    }
}

/// Encrypt plaintext with AES-256-GCM, prepending the nonce to the ciphertext
pub fn encrypt(plaintext: &[u8], enc: &mut FeiqEncryptor) -> Result<Vec<u8>, ring::error::Unspecified> {
    let nonce = enc.nonce_seq.advance()?;
    let nonce_bytes = nonce.as_ref().to_vec();
    let unbound = UnboundKey::new(&AES_256_GCM, &enc.key_bytes)?;

    // Use LessSafeKey which takes individual nonces
    let key = ring::aead::LessSafeKey::new(unbound);
    let mut in_out = plaintext.to_vec();
    // seal_in_place_append_tag extends in_out with tag bytes internally
    key.seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)?;
    // Prepend 12-byte nonce so decrypt is robust against packet loss
    let mut result = Vec::with_capacity(NONCE_LEN + in_out.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend(in_out);
    Ok(result)
}

/// Decrypt ciphertext with AES-256-GCM, extracting the nonce from the prefix
pub fn decrypt(ciphertext: &[u8], dec: &mut FeiqDecryptor) -> Result<Vec<u8>, ring::error::Unspecified> {
    if ciphertext.len() < NONCE_LEN {
        return Err(ring::error::Unspecified);
    }
    let (nonce_bytes, ct) = ciphertext.split_at(NONCE_LEN);
    let mut nonce_arr = [0u8; NONCE_LEN];
    nonce_arr.copy_from_slice(nonce_bytes);
    let nonce = Nonce::assume_unique_for_key(nonce_arr);
    let unbound = UnboundKey::new(&AES_256_GCM, &dec.key_bytes)?;

    let key = ring::aead::LessSafeKey::new(unbound);
    let mut in_out = ct.to_vec();
    let plaintext = key.open_in_place(nonce, Aad::empty(), &mut in_out)?;
    Ok(plaintext.to_vec())
}

/// Detect feiq++ peer
pub fn is_feiq_plus_plus(version: &str) -> bool {
    version.starts_with("feiq_plus_plus")
}

/// Generate a keypair for LONG-LIVED storage (broadcast keypair).
/// Returns (private_key_bytes, public_key_bytes) as clonable [u8; 32] + Vec<u8>.
///
/// Unlike `generate_keypair()` which returns a ring::agreement::EphemeralPrivateKey
/// (single-use, not Clone), this returns raw bytes that can be cloned and used
/// for MULTIPLE ECDH operations with different peers (e.g., FellowAnsEntry).
pub fn generate_broadcast_keypair() -> ([u8; 32], Vec<u8>) {
    let rng = SystemRandom::new();
    let mut private = [0u8; 32];
    // ring::SystemRandom implements SecureRandom; fill never fails in practice
    rng.fill(&mut private).expect("OS RNG should not fail during key generation");

    // X25519 clamping (x25519-dalek also clamps internally, but be explicit)
    private[0] &= 248;
    private[31] &= 127;
    private[31] |= 64;

    let public = x25519(private, X25519_BASEPOINT_BYTES);
    (private, public.to_vec())
}

/// Compute ECDH shared secret from raw (clonable) private key bytes.
///
/// Unlike `compute_shared_secret()` which consumes an `EphemeralPrivateKey`
/// (single-use), this function takes a reference to the private key bytes,
/// allowing the SAME private key to be used for ECDH with MULTIPLE peers.
pub fn compute_shared_secret_from_raw(
    private_key: &[u8; 32],
    peer_public: &[u8],
) -> Result<Vec<u8>, ring::error::Unspecified> {
    if peer_public.len() < 32 {
        return Err(ring::error::Unspecified);
    }
    let mut peer_pub = [0u8; 32];
    peer_pub.copy_from_slice(&peer_public[..32]);
    let shared = x25519(*private_key, peer_pub);
    Ok(shared.to_vec())
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
        assert_eq!(ct.len(), NONCE_LEN + msg.len() + TAG_LEN); // nonce + message + 16-byte tag
        let pt = decrypt(&ct, &mut dec).unwrap();
        assert_eq!(&pt, msg);
    }

    #[test]
    fn test_detect() {
        assert!(is_feiq_plus_plus("feiq_plus_plus#128#MAC#0#0#0#1#9"));
        assert!(!is_feiq_plus_plus("1_lbt6_0#128#MAC#0#0#0#4001#9"));
    }
}
