#![allow(dead_code)]

use std::{cell::Cell, str::FromStr};

use assay_identity::{
    Audience, ClientId, Clock, EntropySource, IssuerUrl, OidcDeploymentConfig,
    OidcDeploymentConfigBuilder, RedirectUri, SignatureError, SignatureVerifier, SigningAlgorithm,
    UnixTime, UpstreamIdToken, VerifiedAssertion,
};
use sha2::{Digest, Sha256};

pub const ISSUER: &str = "https://issuer.example";
pub const CLIENT_ID: &str = "assay-web-client";
pub const AUDIENCE: &str = "assay-api";
pub const REDIRECT: &str = "https://assay.example/auth/oidc/callback";

pub fn config() -> OidcDeploymentConfig {
    OidcDeploymentConfigBuilder::new(
        IssuerUrl::from_str(ISSUER).unwrap(),
        ClientId::from_str(CLIENT_ID).unwrap(),
        Audience::from_str(AUDIENCE).unwrap(),
    )
    .allow_redirect(RedirectUri::from_str(REDIRECT).unwrap())
    .allow_algorithm(SigningAlgorithm::Rs256)
    .scope("openid")
    .scope("profile")
    .clock_leeway_seconds(60)
    .build()
    .unwrap()
}

pub struct FixedClock(pub i64);

impl Clock for FixedClock {
    fn now(&self) -> UnixTime {
        UnixTime::from_seconds(self.0)
    }
}

/// Deterministic entropy: a small LCG yields distinct bytes per call and position.
pub struct SeqEntropy {
    state: Cell<u64>,
}

impl SeqEntropy {
    pub fn new(seed: u64) -> Self {
        Self {
            state: Cell::new(seed),
        }
    }
}

impl EntropySource for SeqEntropy {
    fn fill(&self, buffer: &mut [u8]) {
        let mut value = self.state.get();
        for slot in buffer.iter_mut() {
            value = value
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *slot = (value >> 33) as u8;
        }
        self.state.set(value);
    }
}

/// Deterministic signature verifier returning a preset assertion or failure.
pub struct FakeVerifier {
    result: Result<VerifiedAssertion, SignatureError>,
}

impl FakeVerifier {
    pub fn proving(algorithm: SigningAlgorithm, claims: assay_identity::VerifiedClaims) -> Self {
        Self {
            result: Ok(VerifiedAssertion::new(algorithm, claims)),
        }
    }

    pub fn failing(error: SignatureError) -> Self {
        Self { result: Err(error) }
    }
}

impl SignatureVerifier for FakeVerifier {
    fn verify(&self, _token: &UpstreamIdToken) -> Result<VerifiedAssertion, SignatureError> {
        self.result.clone()
    }
}

/// Recomputes the PKCE `S256` challenge to assert the verifier binds to it.
pub fn pkce_s256(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64url_no_pad(&digest)
}

fn base64url_no_pad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        out.push(ALPHABET[b0 >> 2] as char);
        out.push(ALPHABET[((b0 & 0b11) << 4) | (b1 >> 4)] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((b1 & 0b1111) << 2) | (b2 >> 6)] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[b2 & 0b111111] as char);
        }
    }
    out
}
