//! Built-in v1 path classification dispatcher.
//!
//! Applies the v1 path matchers in precedence order and emits the matching
//! rule identifier, category, and confidence. The matchers themselves live in
//! `matchers.rs`; this module owns only the precedence and rule identifiers.

use crate::{
    categories::{ClassificationCategory, ClassificationTag},
    confidence::Confidence,
    decision::ClassificationDecision,
    matchers::{
        is_build_output, is_ci, is_configuration, is_coverage, is_dependency_manifest,
        is_documentation, is_generated, is_infrastructure, is_lockfile, is_minified,
        is_schema_migration, is_security_policy, is_source, is_test, is_vendored,
    },
    path::PortablePath,
};

pub(crate) fn classify_path_v1(
    path: &PortablePath,
    generated_rules_enabled: bool,
    vendored_rules_enabled: bool,
) -> ClassificationDecision {
    let components = path.lowercase_components();
    let filename = components.last().map(String::as_str).unwrap_or_default();

    if is_coverage(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Coverage,
            "classifier.v1.coverage",
            Confidence::HIGH,
        );
    }
    if is_build_output(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::BuildOutput,
            "classifier.v1.build_output",
            Confidence::HIGH,
        );
    }
    if generated_rules_enabled && is_generated(&components, filename) {
        let mut decision = ClassificationDecision::built_in(
            ClassificationCategory::Generated,
            "classifier.v1.generated",
            Confidence::HIGH,
        );
        if is_minified(filename) {
            decision = decision.tagged(ClassificationTag::Minified);
        }
        return decision;
    }
    if vendored_rules_enabled && is_vendored(&components) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Vendored,
            "classifier.v1.vendored",
            Confidence::HIGH,
        );
    }
    if is_ci(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::CiCd,
            "classifier.v1.ci_cd",
            Confidence::HIGH,
        );
    }
    if is_schema_migration(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::SchemaMigration,
            "classifier.v1.schema_migration",
            Confidence::HIGH,
        );
    }
    if is_security_policy(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::SecurityPolicy,
            "classifier.v1.security_policy",
            Confidence::HIGH,
        );
    }
    if is_documentation(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Documentation,
            "classifier.v1.documentation",
            Confidence::HIGH,
        );
    }
    if is_test(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Test,
            "classifier.v1.test",
            Confidence::HIGH,
        );
    }
    if is_lockfile(filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Dependency,
            "classifier.v1.dependency.lockfile",
            Confidence::HIGH,
        )
        .tagged(ClassificationTag::Lockfile);
    }
    if is_dependency_manifest(filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Dependency,
            "classifier.v1.dependency.manifest",
            Confidence::HIGH,
        )
        .tagged(ClassificationTag::DependencyManifest);
    }
    if is_infrastructure(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Infrastructure,
            "classifier.v1.infrastructure",
            Confidence::HIGH,
        );
    }
    if is_configuration(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Configuration,
            "classifier.v1.configuration",
            Confidence::MEDIUM,
        );
    }
    if is_source(filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::ProductionCode,
            "classifier.v1.production_code",
            Confidence::MEDIUM_HIGH,
        );
    }
    ClassificationDecision::built_in(
        ClassificationCategory::Unknown,
        "classifier.v1.unknown",
        Confidence::LOW,
    )
}
