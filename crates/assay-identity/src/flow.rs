use std::{collections::BTreeMap, fmt};

use sha2::{Digest, Sha256};

use crate::{
    config::OidcDeploymentConfig, encoding::base64url_no_pad, values::RedirectUri,
    verification::constant_time_eq,
};

const SECRET_BYTES: usize = 32;

/// Injected entropy port. A deterministic source keeps flow tests reproducible.
pub trait EntropySource {
    /// Fills the buffer with cryptographically strong bytes in production.
    fn fill(&self, buffer: &mut [u8]);
}

fn generate_secret(entropy: &dyn EntropySource) -> String {
    let mut bytes = [0u8; SECRET_BYTES];
    entropy.fill(&mut bytes);
    base64url_no_pad(&bytes)
}

/// The opaque CSRF `state`. Secret material; never derives Debug, Display, or serde.
#[derive(Clone)]
pub struct State(String);

impl State {
    fn reveal(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for State {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("State(<redacted>)")
    }
}

/// The opaque replay-binding `nonce`. Secret material; carried into token validation.
#[derive(Clone)]
pub struct Nonce(String);

impl Nonce {
    /// Reveals the nonce only to bind it against the returned id token.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Nonce {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Nonce(<redacted>)")
    }
}

/// The PKCE code verifier. Secret material sent only during server-side code exchange.
#[derive(Clone)]
pub struct PkceVerifier(String);

impl PkceVerifier {
    /// Reveals the verifier only to the server-side token-exchange request.
    pub fn reveal(&self) -> &str {
        &self.0
    }

    fn challenge(&self) -> PkceChallenge {
        let digest = Sha256::digest(self.0.as_bytes());
        PkceChallenge(base64url_no_pad(&digest))
    }
}

impl fmt::Debug for PkceVerifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PkceVerifier(<redacted>)")
    }
}

/// The public PKCE `S256` code challenge derived from the secret verifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PkceChallenge(String);

impl PkceChallenge {
    /// Returns the code-challenge value placed in the authorization request.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The outgoing authorization-request parameters. No upstream browser token exists yet.
#[derive(Clone, Debug)]
pub struct AuthorizationRedirect {
    state: String,
    nonce: String,
    redirect_uri: RedirectUri,
    code_challenge: PkceChallenge,
}

impl AuthorizationRedirect {
    /// Returns the opaque state to place in the authorization request.
    pub fn state(&self) -> &str {
        &self.state
    }

    /// Returns the nonce to place in the authorization request.
    pub fn nonce(&self) -> &str {
        &self.nonce
    }

    /// Returns the exact redirect URI drawn from the deployment allowlist.
    pub const fn redirect_uri(&self) -> &RedirectUri {
        &self.redirect_uri
    }

    /// Returns the `S256` code challenge.
    pub const fn code_challenge(&self) -> &PkceChallenge {
        &self.code_challenge
    }

    /// Returns the fixed code-challenge method.
    pub const fn code_challenge_method(&self) -> &'static str {
        "S256"
    }
}

/// Callback parameters returned by the browser to the redirect endpoint.
#[derive(Clone, Debug)]
pub struct CallbackParams {
    state: String,
    code: String,
    redirect_uri: String,
}

impl CallbackParams {
    /// Wraps the raw browser-supplied callback parameters.
    pub fn new(state: &str, code: &str, redirect_uri: &str) -> Self {
        Self {
            state: state.to_owned(),
            code: code.to_owned(),
            redirect_uri: redirect_uri.to_owned(),
        }
    }
}

/// A validated callback ready for server-side code exchange and token validation.
pub struct VerifiedCallback {
    code: String,
    pkce_verifier: PkceVerifier,
    nonce: Nonce,
}

impl VerifiedCallback {
    /// Returns the opaque authorization code to exchange server-side.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the PKCE verifier to include in the token exchange.
    pub const fn pkce_verifier(&self) -> &PkceVerifier {
        &self.pkce_verifier
    }

    /// Returns the nonce to bind against the returned id token.
    pub const fn nonce(&self) -> &Nonce {
        &self.nonce
    }
}

impl fmt::Debug for VerifiedCallback {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCallback")
            .field("code", &"<redacted>")
            .field("pkce_verifier", &self.pkce_verifier)
            .field("nonce", &self.nonce)
            .finish()
    }
}

/// Redacted authorization-flow failure. Invalid and replayed callbacks fail closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowError {
    RedirectNotAllowed,
    UnknownOrReplayedState,
    RedirectMismatch,
    EmptyCode,
}

struct PendingAuthorization {
    state: State,
    nonce: Nonce,
    pkce_verifier: PkceVerifier,
    redirect_uri: RedirectUri,
}

/// In-memory pending-authorization store enforcing single-use state redemption.
#[derive(Default)]
pub struct AuthorizationStore {
    pending: BTreeMap<String, PendingAuthorization>,
}

impl AuthorizationStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Begins one transaction, storing its secrets and returning the redirect data.
    pub fn begin(
        &mut self,
        config: &OidcDeploymentConfig,
        redirect_uri: &RedirectUri,
        entropy: &dyn EntropySource,
    ) -> Result<AuthorizationRedirect, FlowError> {
        if !config.allows_redirect(redirect_uri) {
            return Err(FlowError::RedirectNotAllowed);
        }
        let state = State(generate_secret(entropy));
        let nonce = Nonce(generate_secret(entropy));
        let pkce_verifier = PkceVerifier(generate_secret(entropy));
        let redirect = AuthorizationRedirect {
            state: state.reveal().to_owned(),
            nonce: nonce.reveal().to_owned(),
            redirect_uri: redirect_uri.clone(),
            code_challenge: pkce_verifier.challenge(),
        };
        self.pending.insert(
            state.reveal().to_owned(),
            PendingAuthorization {
                state,
                nonce,
                pkce_verifier,
                redirect_uri: redirect_uri.clone(),
            },
        );
        Ok(redirect)
    }

    /// Redeems a callback once. A reused or unknown state fails closed.
    pub fn redeem(&mut self, params: &CallbackParams) -> Result<VerifiedCallback, FlowError> {
        let Some(pending) = self.pending.remove(&params.state) else {
            return Err(FlowError::UnknownOrReplayedState);
        };
        if !constant_time_eq(pending.state.reveal().as_bytes(), params.state.as_bytes()) {
            return Err(FlowError::UnknownOrReplayedState);
        }
        if pending.redirect_uri.as_str() != params.redirect_uri {
            return Err(FlowError::RedirectMismatch);
        }
        if params.code.is_empty() {
            return Err(FlowError::EmptyCode);
        }
        Ok(VerifiedCallback {
            code: params.code.clone(),
            pkce_verifier: pending.pkce_verifier,
            nonce: pending.nonce,
        })
    }

    /// Returns the number of outstanding transactions, for store introspection.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}
