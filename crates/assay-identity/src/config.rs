use std::collections::BTreeSet;

use crate::{
    entitlements::EntitlementPolicy,
    policy::AdministratorMappingPolicy,
    values::{Audience, ClientId, IssuerUrl, RedirectUri},
    verification::SigningAlgorithm,
};

const MAX_CLOCK_LEEWAY_SECONDS: i64 = 300;

/// One deployment's provider-agnostic OIDC integration.
/// No provider domain, claim convention, or role enum is hard-coded here.
#[derive(Clone, Debug)]
pub struct OidcDeploymentConfig {
    issuer: IssuerUrl,
    client_id: ClientId,
    expected_audience: Audience,
    redirect_allowlist: BTreeSet<RedirectUri>,
    allowed_algorithms: BTreeSet<SigningAlgorithm>,
    scopes: Vec<String>,
    clock_leeway_seconds: i64,
    single_issuer_no_local_registration: bool,
    admin_mapping: AdministratorMappingPolicy,
    entitlement_policy: EntitlementPolicy,
}

impl OidcDeploymentConfig {
    pub const fn issuer(&self) -> &IssuerUrl {
        &self.issuer
    }

    pub const fn client_id(&self) -> &ClientId {
        &self.client_id
    }

    /// Assay-specific audience validated on every token.
    pub const fn expected_audience(&self) -> &Audience {
        &self.expected_audience
    }

    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }

    pub const fn clock_leeway_seconds(&self) -> i64 {
        self.clock_leeway_seconds
    }

    pub fn allows_algorithm(&self, algorithm: SigningAlgorithm) -> bool {
        self.allowed_algorithms.contains(&algorithm)
    }

    /// Whether a redirect URI matches the exact allowlist.
    pub fn allows_redirect(&self, redirect_uri: &RedirectUri) -> bool {
        self.redirect_allowlist.contains(redirect_uri)
    }

    /// Whether this deployment offers independent Assay registration.
    pub const fn allows_local_registration(&self) -> bool {
        !self.single_issuer_no_local_registration
    }

    pub const fn admin_mapping(&self) -> &AdministratorMappingPolicy {
        &self.admin_mapping
    }

    pub const fn entitlement_policy(&self) -> &EntitlementPolicy {
        &self.entitlement_policy
    }
}

/// Redacted configuration-construction error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    NoRedirectUri,
    NoSigningAlgorithm,
    LeewayOutOfRange,
}

/// Builder that validates one immutable deployment configuration.
pub struct OidcDeploymentConfigBuilder {
    issuer: IssuerUrl,
    client_id: ClientId,
    expected_audience: Audience,
    redirect_allowlist: BTreeSet<RedirectUri>,
    allowed_algorithms: BTreeSet<SigningAlgorithm>,
    scopes: Vec<String>,
    clock_leeway_seconds: i64,
    single_issuer_no_local_registration: bool,
    admin_mapping: Option<AdministratorMappingPolicy>,
    entitlement_policy: Option<EntitlementPolicy>,
}

impl OidcDeploymentConfigBuilder {
    pub fn new(issuer: IssuerUrl, client_id: ClientId, expected_audience: Audience) -> Self {
        Self {
            issuer,
            client_id,
            expected_audience,
            redirect_allowlist: BTreeSet::new(),
            allowed_algorithms: BTreeSet::new(),
            scopes: Vec::new(),
            clock_leeway_seconds: 60,
            single_issuer_no_local_registration: false,
            admin_mapping: None,
            entitlement_policy: None,
        }
    }

    #[must_use]
    pub fn allow_redirect(mut self, redirect_uri: RedirectUri) -> Self {
        self.redirect_allowlist.insert(redirect_uri);
        self
    }

    #[must_use]
    pub fn allow_algorithm(mut self, algorithm: SigningAlgorithm) -> Self {
        self.allowed_algorithms.insert(algorithm);
        self
    }

    #[must_use]
    pub fn scope(mut self, scope: &str) -> Self {
        self.scopes.push(scope.to_owned());
        self
    }

    #[must_use]
    pub const fn clock_leeway_seconds(mut self, seconds: i64) -> Self {
        self.clock_leeway_seconds = seconds;
        self
    }

    /// Marks the deployment as single-issuer with no independent registration.
    #[must_use]
    pub const fn single_issuer_no_local_registration(mut self, enabled: bool) -> Self {
        self.single_issuer_no_local_registration = enabled;
        self
    }

    #[must_use]
    pub fn admin_mapping(mut self, policy: AdministratorMappingPolicy) -> Self {
        self.admin_mapping = Some(policy);
        self
    }

    #[must_use]
    pub fn entitlement_policy(mut self, policy: EntitlementPolicy) -> Self {
        self.entitlement_policy = Some(policy);
        self
    }

    pub fn build(self) -> Result<OidcDeploymentConfig, ConfigError> {
        if self.redirect_allowlist.is_empty() {
            return Err(ConfigError::NoRedirectUri);
        }
        if self.allowed_algorithms.is_empty() {
            return Err(ConfigError::NoSigningAlgorithm);
        }
        if !(0..=MAX_CLOCK_LEEWAY_SECONDS).contains(&self.clock_leeway_seconds) {
            return Err(ConfigError::LeewayOutOfRange);
        }
        Ok(OidcDeploymentConfig {
            issuer: self.issuer,
            client_id: self.client_id,
            expected_audience: self.expected_audience,
            redirect_allowlist: self.redirect_allowlist,
            allowed_algorithms: self.allowed_algorithms,
            scopes: self.scopes,
            clock_leeway_seconds: self.clock_leeway_seconds,
            single_issuer_no_local_registration: self.single_issuer_no_local_registration,
            admin_mapping: self
                .admin_mapping
                .unwrap_or_else(|| AdministratorMappingPolicy::none("unset")),
            entitlement_policy: self.entitlement_policy.unwrap_or_default(),
        })
    }
}