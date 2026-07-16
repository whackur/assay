use std::collections::BTreeSet;

use crate::{
    entitlements::EntitlementPolicy,
    policy::AdministratorMappingPolicy,
    values::{Audience, ClientId, IssuerUrl, RedirectUri},
    verification::SigningAlgorithm,
};

const MAX_CLOCK_LEEWAY_SECONDS: i64 = 300;

/// One deployment's provider-agnostic OIDC integration.
///
/// The issuer, audience, client, redirect allowlist, and any trusted admin claim
/// arrive only as configuration values. No provider domain, claim convention, or
/// role enum is hard-coded here.
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
    /// Returns the configured issuer.
    pub const fn issuer(&self) -> &IssuerUrl {
        &self.issuer
    }

    /// Returns the registered Assay client identifier.
    pub const fn client_id(&self) -> &ClientId {
        &self.client_id
    }

    /// Returns the Assay-specific audience validated on every token.
    pub const fn expected_audience(&self) -> &Audience {
        &self.expected_audience
    }

    /// Returns the requested scopes.
    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }

    /// Returns the signature-time leeway in seconds.
    pub const fn clock_leeway_seconds(&self) -> i64 {
        self.clock_leeway_seconds
    }

    /// Returns whether an algorithm is in the allowlist.
    pub fn allows_algorithm(&self, algorithm: SigningAlgorithm) -> bool {
        self.allowed_algorithms.contains(&algorithm)
    }

    /// Returns whether a redirect URI matches the exact allowlist.
    pub fn allows_redirect(&self, redirect_uri: &RedirectUri) -> bool {
        self.redirect_allowlist.contains(redirect_uri)
    }

    /// Returns whether this deployment offers independent Assay registration.
    pub const fn allows_local_registration(&self) -> bool {
        !self.single_issuer_no_local_registration
    }

    /// Returns the administrator-mapping policy.
    pub const fn admin_mapping(&self) -> &AdministratorMappingPolicy {
        &self.admin_mapping
    }

    /// Returns the local entitlement policy.
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
    /// Starts a builder from the mandatory issuer, client, and audience values.
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

    /// Adds one exact redirect URI to the allowlist.
    #[must_use]
    pub fn allow_redirect(mut self, redirect_uri: RedirectUri) -> Self {
        self.redirect_allowlist.insert(redirect_uri);
        self
    }

    /// Adds one allowed asymmetric signing algorithm.
    #[must_use]
    pub fn allow_algorithm(mut self, algorithm: SigningAlgorithm) -> Self {
        self.allowed_algorithms.insert(algorithm);
        self
    }

    /// Requests one scope value.
    #[must_use]
    pub fn scope(mut self, scope: &str) -> Self {
        self.scopes.push(scope.to_owned());
        self
    }

    /// Sets the signature-time leeway in seconds.
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

    /// Sets the administrator-mapping policy.
    #[must_use]
    pub fn admin_mapping(mut self, policy: AdministratorMappingPolicy) -> Self {
        self.admin_mapping = Some(policy);
        self
    }

    /// Sets the local entitlement policy.
    #[must_use]
    pub fn entitlement_policy(mut self, policy: EntitlementPolicy) -> Self {
        self.entitlement_policy = Some(policy);
        self
    }

    /// Validates and builds the configuration.
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
