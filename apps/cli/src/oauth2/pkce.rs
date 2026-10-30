use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::CryptoRng;
use sha2::{Digest, Sha256};

/// Generates a PKCE (Proof Key for Code Exchange, RFC 7636).
///
/// Consists of:
/// - Code challenge: Random `N` bytes, base64url (RFC 4648) encoded
/// - Code verifier:  SHA-256 hashed and base64url encoded code challenge
pub fn generate_s256<const N: usize>(rng: &mut impl CryptoRng) -> (CodeVerifier, CodeChallenge) {
    let verifier = {
        let mut bytes = [0u8; N];
        rng.fill_bytes(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    };
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (CodeVerifier(verifier), CodeChallenge(challenge))
}

pub struct CodeChallenge(String);
pub struct CodeVerifier(String);

impl CodeVerifier {
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl CodeChallenge {
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
    /// "S256" is currently the only recommended PKCE method
    pub fn method_as_str(&self) -> &str {
        "S256"
    }
}

#[cfg(test)]
#[path = "pkce.tests.rs"]
mod tests;
