/// Classified project type, matching the public evaluation contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectType {
    Application,
    LibrarySdkFramework,
    CliDeveloperTool,
    ServiceInfrastructurePlatform,
    CuratedResource,
    ProtocolSpecificationStandard,
    DatasetModelResearchArtifact,
    EducationalExampleTemplate,
    ExperimentalProofOfConcept,
}

impl ProjectType {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Application => "application",
            Self::LibrarySdkFramework => "library_sdk_framework",
            Self::CliDeveloperTool => "cli_developer_tool",
            Self::ServiceInfrastructurePlatform => "service_infrastructure_platform",
            Self::CuratedResource => "curated_resource",
            Self::ProtocolSpecificationStandard => "protocol_specification_standard",
            Self::DatasetModelResearchArtifact => "dataset_model_research_artifact",
            Self::EducationalExampleTemplate => "educational_example_template",
            Self::ExperimentalProofOfConcept => "experimental_proof_of_concept",
        }
    }
}

/// Classified project maturity, matching the public evaluation contract.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProjectMaturity {
    Concept,
    Prototype,
    Alpha,
    Beta,
    Stable,
    Maintenance,
    Dormant,
    Archived,
}

impl ProjectMaturity {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Concept => "concept",
            Self::Prototype => "prototype",
            Self::Alpha => "alpha",
            Self::Beta => "beta",
            Self::Stable => "stable",
            Self::Maintenance => "maintenance",
            Self::Dormant => "dormant",
            Self::Archived => "archived",
        }
    }
}

/// Credential-independent provider identity recorded on the evaluation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EvaluatorProvider {
    Deterministic,
    OpenaiApi,
    OllamaCompatible,
    CodexCli,
    CodexOauth,
}

impl EvaluatorProvider {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::OpenaiApi => "openai_api",
            Self::OllamaCompatible => "ollama_compatible",
            Self::CodexCli => "codex_cli",
            Self::CodexOauth => "codex_oauth",
        }
    }
}

/// Publication scope recorded on the evaluation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Visibility {
    Public,
    PrivatePreview,
    PrivateLocal,
}

impl Visibility {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::PrivatePreview => "private_preview",
            Self::PrivateLocal => "private_local",
        }
    }
}
