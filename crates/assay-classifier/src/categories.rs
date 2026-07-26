//! Primary categories and secondary tags emitted by a classification policy.

/// Primary role assigned to one repository file.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ClassificationCategory {
    ProductionCode,
    Test,
    Documentation,
    /// Continuous integration and delivery configuration.
    CiCd,
    /// Deployment and infrastructure-as-code material.
    Infrastructure,
    /// Versioned database or schema migration material.
    SchemaMigration,
    Dependency,
    /// Repository security policy or security automation configuration.
    SecurityPolicy,
    Configuration,
    Generated,
    Vendored,
    BuildOutput,
    Coverage,
    /// No built-in path rule supplied sufficient evidence.
    Unknown,
}

/// Optional facts retained alongside the primary category.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ClassificationTag {
    DependencyManifest,
    Lockfile,
    /// `linguist-generated=true` contributed to classification.
    LinguistGenerated,
    /// `linguist-vendored=true` contributed to classification.
    LinguistVendored,
    /// `linguist-generated=false` suppressed a matching built-in rule.
    GeneratedSuppressed,
    /// `linguist-vendored=false` suppressed a matching built-in rule.
    VendoredSuppressed,
    /// Resolved Git attribute facts were unavailable.
    AttributesUnavailable,
    /// Minified filename supplied generated-file evidence.
    Minified,
}