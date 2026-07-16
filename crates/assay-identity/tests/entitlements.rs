use std::collections::{BTreeMap, BTreeSet};

use assay_identity::{Entitlement, EntitlementPolicy, LocalRole};

#[test]
fn default_member_gets_features_but_no_admin_operations() {
    let policy = EntitlementPolicy::default();
    assert!(policy.authorizes(LocalRole::Member, Entitlement::AnalysisPublicSubmit));
    assert!(policy.authorizes(LocalRole::Member, Entitlement::ProjectSave));
    assert!(!policy.authorizes(LocalRole::Member, Entitlement::AnalysisAdminRerun));
    assert!(!policy.authorizes(LocalRole::Member, Entitlement::AnalysisAdminDelete));
}

#[test]
fn default_administrator_adds_admin_operations() {
    let policy = EntitlementPolicy::default();
    assert!(policy.authorizes(LocalRole::Administrator, Entitlement::AnalysisAdminRerun));
    assert!(policy.authorizes(LocalRole::Administrator, Entitlement::AnalysisAdminDelete));
    assert!(policy.authorizes(LocalRole::Administrator, Entitlement::AnalysisPublicSubmit));
}

#[test]
fn a_deployment_configures_bundles_explicitly() {
    let policy = EntitlementPolicy::new(BTreeMap::from([(
        LocalRole::Member,
        BTreeSet::from([Entitlement::AnalysisCompare]),
    )]));
    assert!(policy.authorizes(LocalRole::Member, Entitlement::AnalysisCompare));
    assert!(!policy.authorizes(LocalRole::Member, Entitlement::AnalysisPublicSubmit));
    assert!(!policy.authorizes(LocalRole::Administrator, Entitlement::AnalysisAdminRerun));
    assert_eq!(policy.entitlements(LocalRole::Administrator), []);
}

#[test]
fn entitlement_codes_are_stable_dotted_identifiers() {
    assert_eq!(
        Entitlement::AnalysisAdminRerun.code(),
        "analysis.admin.rerun"
    );
    assert_eq!(Entitlement::CatalogSubmit.code(), "catalog.submit");
}
