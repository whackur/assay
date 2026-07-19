use crate::ProjectMaturity;
use crate::ProjectType;

/// A cited observation that a project exhibits one type-relevant signal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeSignal {
    CuratedListStructure,
    SpecificationDocument,
    DatasetOrModelArtifact,
    TemplateMarker,
    ProofOfConceptMarker,
    ServiceDeploymentDeclared,
    CliEntrypointDeclared,
    LibraryPackagingDeclared,
    ApplicationEntrypointDeclared,
    PluginHostDeclared,
    FrameworkExtensionPoints,
    DevOpsInfrastructure,
}

impl TypeSignal {
    /// Returns the primary type this signal implies, when it maps to one.
    pub(crate) const fn primary_type(self) -> Option<ProjectType> {
        match self {
            Self::CuratedListStructure => Some(ProjectType::CuratedResource),
            Self::SpecificationDocument => Some(ProjectType::ProtocolSpecificationStandard),
            Self::DatasetOrModelArtifact => Some(ProjectType::DatasetModelResearchArtifact),
            Self::TemplateMarker => Some(ProjectType::EducationalExampleTemplate),
            Self::ProofOfConceptMarker => Some(ProjectType::ExperimentalProofOfConcept),
            Self::ServiceDeploymentDeclared => Some(ProjectType::ServiceInfrastructurePlatform),
            Self::CliEntrypointDeclared => Some(ProjectType::CliDeveloperTool),
            Self::LibraryPackagingDeclared => Some(ProjectType::LibrarySdkFramework),
            Self::ApplicationEntrypointDeclared => Some(ProjectType::Application),
            Self::PluginHostDeclared
            | Self::FrameworkExtensionPoints
            | Self::DevOpsInfrastructure => None,
        }
    }

    /// Returns the descriptive tag this signal contributes, when any.
    pub(crate) const fn tag(self) -> Option<&'static str> {
        match self {
            Self::PluginHostDeclared => Some("plugin"),
            Self::FrameworkExtensionPoints => Some("framework"),
            Self::DevOpsInfrastructure => Some("infrastructure"),
            _ => None,
        }
    }
}

/// Priority order that resolves the primary type when several types are implied.
///
/// Specific artifact kinds win over general delivery forms; among delivery
/// forms the most operationally specific declaration wins. This order is
/// versioned policy data, not an incidental match order.
pub(crate) const TYPE_PRIORITY: [ProjectType; 9] = [
    ProjectType::CuratedResource,
    ProjectType::ProtocolSpecificationStandard,
    ProjectType::DatasetModelResearchArtifact,
    ProjectType::EducationalExampleTemplate,
    ProjectType::ExperimentalProofOfConcept,
    ProjectType::ServiceInfrastructurePlatform,
    ProjectType::CliDeveloperTool,
    ProjectType::LibrarySdkFramework,
    ProjectType::Application,
];

pub(crate) const DELIVERY_FORM_TYPES: [ProjectType; 4] = [
    ProjectType::ServiceInfrastructurePlatform,
    ProjectType::CliDeveloperTool,
    ProjectType::LibrarySdkFramework,
    ProjectType::Application,
];

/// A cited observation that a project exhibits one maturity-relevant signal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MaturitySignal {
    ArchivedRepository,
    MaintenanceModeDeclared,
    StableReleaseTagged,
    BetaPrereleaseTagged,
    AlphaPrereleaseTagged,
    SustainedIteration,
    SingleInitialImport,
    ConceptOnly,
    DormantNoRecentActivity,
}

impl MaturitySignal {
    pub(crate) const fn maturity(self) -> ProjectMaturity {
        match self {
            Self::ArchivedRepository => ProjectMaturity::Archived,
            Self::MaintenanceModeDeclared => ProjectMaturity::Maintenance,
            Self::StableReleaseTagged => ProjectMaturity::Stable,
            Self::BetaPrereleaseTagged => ProjectMaturity::Beta,
            Self::AlphaPrereleaseTagged => ProjectMaturity::Alpha,
            Self::SustainedIteration => ProjectMaturity::Prototype,
            Self::SingleInitialImport | Self::ConceptOnly => ProjectMaturity::Concept,
            Self::DormantNoRecentActivity => ProjectMaturity::Dormant,
        }
    }
}

/// Maturity resolution priority. A stronger lifecycle or release signal wins
/// over inactivity so a stable-but-quiet project is not misread as dormant.
pub(crate) const MATURITY_PRIORITY: [MaturitySignal; 9] = [
    MaturitySignal::ArchivedRepository,
    MaturitySignal::MaintenanceModeDeclared,
    MaturitySignal::StableReleaseTagged,
    MaturitySignal::BetaPrereleaseTagged,
    MaturitySignal::AlphaPrereleaseTagged,
    MaturitySignal::SustainedIteration,
    MaturitySignal::SingleInitialImport,
    MaturitySignal::ConceptOnly,
    MaturitySignal::DormantNoRecentActivity,
];
