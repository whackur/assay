//! Primary categories and secondary tags emitted by a classification policy.

/// Primary role assigned to one repository file.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ClassificationCategory {
    /// Application, library, or other implementation source.
    ProductionCode,
    /// Automated or manual test source and fixtures.
    Test,
    /// Documentation and community-facing text.
    Documentation,
    /// Continuous integration and delivery configuration.
    CiCd,
    /// Deployment and infrastructure-as-code material.
    Infrastructure,
    /// Versioned database or schema migration material.
    SchemaMigration,
    /// Dependency manifests and lockfiles.
    Dependency,
    /// Repository security policy or security automation configuration.
    SecurityPolicy,
    /// General project or tool configuration.
    Configuration,
    /// Generated or minified material.
    Generated,
    /// Vendored or third-party material.
    Vendored,
    /// Build output or compiled artifacts.
    BuildOutput,
    /// Coverage output.
    Coverage,
    /// No built-in path rule supplied sufficient evidence.
    Unknown,
}

/// Optional facts retained alongside the primary category.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ClassificationTag {
    /// The file is a dependency manifest.
    DependencyManifest,
    /// The file is a dependency lockfile.
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
    /// A minified filename supplied generated-file evidence.
    Minified,
}
