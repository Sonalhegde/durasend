use anyhow::Result;
use orion::aead::SecretKey;

/// Derives a 32-byte AEAD secret key from the passphrase using BLAKE3.
/// Note: As described in the architecture document, this direct hash is an MVP key derivation
/// without salt or work factor. Production upgrades should use Argon2id.
pub fn derive_key(passphrase: &str) -> Result<SecretKey> {
    let hash = blake3::hash(passphrase.as_bytes());
    SecretKey::from_slice(hash.as_bytes())
        .map_err(|e| anyhow::anyhow!("Failed to create secret key: {:?}", e))
}

/// Encrypts plaintext using orion's XChaCha20-Poly1305 AEAD seal.
/// Orion automatically generates and prepends a secure random nonce and appends the authentication tag.
pub fn encrypt_chunk(data: &[u8], key: &SecretKey) -> Result<Vec<u8>> {
    orion::aead::seal(key, data)
        .map_err(|e| anyhow::anyhow!("Encryption failed: {:?}", e))
}

/// Decrypts ciphertext using orion's XChaCha20-Poly1305 AEAD open.
/// Orion verifies the authentication tag and extracts the plaintext.
pub fn decrypt_chunk(ciphertext: &[u8], key: &SecretKey) -> Result<Vec<u8>> {
    orion::aead::open(key, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed (auth tag mismatch or corrupted chunk): {:?}", e))
}
