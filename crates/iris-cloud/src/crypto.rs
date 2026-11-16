//! Venice wire format; primitives are implemented by RustCrypto, not by Iris.
use crate::{Error, Result};
type Aes256Gcm = aes_gcm::AesGcm<aes::Aes256, aes_gcm::aead::consts::U12>;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Nonce,
};
use hkdf::Hkdf;
use k256::{ecdh::EphemeralSecret, elliptic_curve::sec1::ToEncodedPoint, PublicKey};
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use sha3::{Digest, Keccak256};
use zeroize::Zeroizing;

pub(crate) fn public_key(text: &str) -> Result<PublicKey> {
    let text = text.strip_prefix("0x").unwrap_or(text);
    if text.len() != 128 && !(text.len() == 130 && text.starts_with("04")) {
        return Err(Error::Attestation);
    }
    let bytes = hex::decode(text).map_err(|_| Error::Attestation)?;
    let bytes = if bytes.len() == 64 {
        [&[4][..], &bytes].concat()
    } else {
        bytes
    };
    PublicKey::from_sec1_bytes(&bytes).map_err(|_| Error::Attestation)
}

pub(crate) fn address(key: &PublicKey) -> [u8; 20] {
    let encoded = key.to_encoded_point(false);
    Keccak256::digest(&encoded.as_bytes()[1..])[12..]
        .try_into()
        .expect("20 bytes")
}

fn aes_key(shared: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let mut key = Zeroizing::new([0; 32]);
    Hkdf::<Sha256>::new(None, shared)
        .expand(b"ecdsa_encryption", key.as_mut())
        .map_err(|_| Error::Encryption)?;
    Ok(key)
}

pub(crate) fn encrypt(plain: &str, recipient: &PublicKey) -> Result<String> {
    let ephemeral = EphemeralSecret::random(&mut OsRng);
    let shared = ephemeral.diffie_hellman(recipient);
    let key = aes_key(shared.raw_secret_bytes())?;
    let mut nonce = [0; 12];
    OsRng.fill_bytes(&mut nonce);
    let encrypted = Aes256Gcm::new_from_slice(key.as_ref())
        .map_err(|_| Error::Encryption)?
        .encrypt(Nonce::from_slice(&nonce), plain.as_bytes())
        .map_err(|_| Error::Encryption)?;
    let mut wire = ephemeral
        .public_key()
        .to_encoded_point(false)
        .as_bytes()
        .to_vec();
    wire.extend_from_slice(&nonce);
    wire.extend_from_slice(&encrypted);
    Ok(hex::encode(wire))
}

/// Per-request recipient secret. Its scalar and shared secrets are zeroized on drop by k256.
pub(crate) struct Recipient(EphemeralSecret);
impl Recipient {
    pub fn new() -> Self {
        Self(EphemeralSecret::random(&mut OsRng))
    }
    pub fn public_hex(&self) -> String {
        hex::encode(self.0.public_key().to_encoded_point(false).as_bytes())
    }
    pub fn decrypt(&self, text: &str) -> Result<String> {
        if text.len() < 186
            || text.len() > crate::MAX_REPLY_BYTES * 2
            || !text.len().is_multiple_of(2)
        {
            return Err(Error::Decryption);
        }
        let raw = hex::decode(text).map_err(|_| Error::Decryption)?;
        if raw[0] != 4 {
            return Err(Error::Decryption);
        }
        let peer = PublicKey::from_sec1_bytes(&raw[..65]).map_err(|_| Error::Decryption)?;
        let shared = self.0.diffie_hellman(&peer);
        let key = aes_key(shared.raw_secret_bytes())?;
        let plain = Zeroizing::new(
            Aes256Gcm::new_from_slice(key.as_ref())
                .map_err(|_| Error::Decryption)?
                .decrypt(Nonce::from_slice(&raw[65..77]), &raw[77..])
                .map_err(|_| Error::Decryption)?,
        );
        String::from_utf8(plain.to_vec()).map_err(|_| Error::Decryption)
    }
}

pub(crate) fn nonce() -> String {
    let mut bytes = [0; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub(crate) fn verify_signature(text: &str, signature: &str, key: &PublicKey) -> Result<()> {
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
    let bytes = hex::decode(signature.strip_prefix("0x").unwrap_or(signature))
        .map_err(|_| Error::Signature)?;
    if bytes.len() != 65 {
        return Err(Error::Signature);
    }
    let recovery = match bytes[64] {
        0 | 1 => bytes[64],
        27 | 28 => bytes[64] - 27,
        _ => return Err(Error::Signature),
    };
    let signature = Signature::from_slice(&bytes[..64]).map_err(|_| Error::Signature)?;
    let prefix = format!("\x19Ethereum Signed Message:\n{}", text.len());
    let digest = Keccak256::new()
        .chain_update(prefix.as_bytes())
        .chain_update(text.as_bytes());
    let recovered = VerifyingKey::recover_from_digest(
        digest,
        &signature,
        RecoveryId::try_from(recovery).map_err(|_| Error::Signature)?,
    )
    .map_err(|_| Error::Signature)?;
    if recovered.to_encoded_point(false) != key.to_encoded_point(false) {
        return Err(Error::Signature);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn agrees_with_independent_python_cryptography_vector() {
        struct ScalarTwo;
        impl RngCore for ScalarTwo {
            fn next_u32(&mut self) -> u32 {
                2
            }
            fn next_u64(&mut self) -> u64 {
                2
            }
            fn fill_bytes(&mut self, bytes: &mut [u8]) {
                bytes.fill(0);
                *bytes.last_mut().unwrap() = 2;
            }
            fn try_fill_bytes(
                &mut self,
                bytes: &mut [u8],
            ) -> std::result::Result<(), rand_core::Error> {
                self.fill_bytes(bytes);
                Ok(())
            }
        }
        impl rand_core::CryptoRng for ScalarTwo {}
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/venice-wire.json")).unwrap();
        let recipient = Recipient(EphemeralSecret::random(&mut ScalarTwo));
        assert_eq!(
            recipient
                .decrypt(fixture["wire"].as_str().unwrap())
                .unwrap(),
            fixture["plaintext"].as_str().unwrap()
        );
    }
    #[test]
    fn authenticated_wire_roundtrip_and_tampering() {
        let recipient = Recipient::new();
        let public = public_key(&recipient.public_hex()).unwrap();
        let text = "Grüße — a private reminder";
        let wire = encrypt(text, &public).unwrap();
        assert_eq!(recipient.decrypt(&wire).unwrap(), text);
        assert_ne!(wire, encrypt(text, &public).unwrap());
        let mut bad = hex::decode(&wire).unwrap();
        *bad.last_mut().unwrap() ^= 1;
        assert!(recipient.decrypt(&hex::encode(bad)).is_err());
        assert!(Recipient::new().decrypt(&wire).is_err());
        assert!(recipient
            .decrypt("plaintext must never be accepted")
            .is_err());
        assert!(public_key(&"00".repeat(65)).is_err());
    }
    #[test]
    fn signature_authenticates_exact_utf8_and_signer() {
        use k256::ecdsa::SigningKey;
        let signer = SigningKey::random(&mut OsRng);
        let text = "Grüße";
        let prefix = format!("\x19Ethereum Signed Message:\n{}", text.len());
        let (sig, recovery) = signer
            .sign_digest_recoverable(Keccak256::new().chain_update(prefix).chain_update(text))
            .unwrap();
        let mut wire = sig.to_bytes().to_vec();
        wire.push(recovery.to_byte() + 27);
        let public =
            PublicKey::from_sec1_bytes(signer.verifying_key().to_encoded_point(false).as_bytes())
                .unwrap();
        assert!(verify_signature(text, &hex::encode(&wire), &public).is_ok());
        assert!(verify_signature("altered", &hex::encode(&wire), &public).is_err());
    }
}
