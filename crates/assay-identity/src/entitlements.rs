use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// A local Assay role. Authorization is always local, never an upstream role.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalRole {
    Member,
    Administrator,
}

/// A local feature entitlement. API handlers authorize the specific action.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Entitlement {
    AnalysisPublicSubmit,
    AnalysisPrivateCreate,
    AnalysisCompare,
    ProjectSave,
    ProjectWatch,
    ReportExport,
    NotificationManage,
    ProviderCodexConnect,
    TokenAgentCreate,
    CatalogSubmit,
    AnalysisAdminRerun,
    AnalysisAdminDelete,
}

impl Entitlement {
    /// Returns the stable dotted entitlement identifier.
    pub const fn code(self) -> &'static str {
        match self {
            Self::AnalysisPublicSubmit => "analysis.public.submit",
            Self::AnalysisPrivateCreate => "analysis.private.create",
            Self::AnalysisCompare => "analysis.compare",
            Self::ProjectSave => "project.save",
            Self::ProjectWatch => "project.watch",
            Self::ReportExport => "report.export",
            Self::NotificationManage => "notification.manage",
            Self::ProviderCodexConnect => "provider.codex.connect",
            Self::TokenAgentCreate => "token.agent.create",
            Self::CatalogSubmit => "catalog.submit",
            Self::AnalysisAdminRerun => "analysis.admin.rerun",
            Self::AnalysisAdminDelete => "analysis.admin.delete",
        }
    }
}

/// Deployment-configured role-to-entitlement bundles evaluated locally.
#[derive(Clone, Debug)]
pub struct EntitlementPolicy {
    grants: BTreeMap<LocalRole, BTreeSet<Entitlement>>,
}

impl EntitlementPolicy {
    /// Builds a policy from explicit role bundles.
    pub fn new(grants: BTreeMap<LocalRole, BTreeSet<Entitlement>>) -> Self {
        Self { grants }
    }

    /// Authorizes one action for a role against the configured bundle.
    pub fn authorizes(&self, role: LocalRole, entitlement: Entitlement) -> bool {
        self.grants
            .get(&role)
            .is_some_and(|set| set.contains(&entitlement))
    }

    /// Returns the entitlements granted to a role in canonical order.
    pub fn entitlements(&self, role: LocalRole) -> Vec<Entitlement> {
        self.grants
            .get(&role)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }
}

impl Default for EntitlementPolicy {
    /// The initial-product bundles: members get non-admin features, admins add operations.
    fn default() -> Self {
        let member = BTreeSet::from([
            Entitlement::AnalysisPublicSubmit,
            Entitlement::AnalysisPrivateCreate,
            Entitlement::AnalysisCompare,
            Entitlement::ProjectSave,
            Entitlement::ProjectWatch,
            Entitlement::ReportExport,
            Entitlement::NotificationManage,
            Entitlement::ProviderCodexConnect,
            Entitlement::TokenAgentCreate,
            Entitlement::CatalogSubmit,
        ]);
        let mut administrator = member.clone();
        administrator.insert(Entitlement::AnalysisAdminRerun);
        administrator.insert(Entitlement::AnalysisAdminDelete);
        Self::new(BTreeMap::from([
            (LocalRole::Member, member),
            (LocalRole::Administrator, administrator),
        ]))
    }
}
