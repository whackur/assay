use std::str::FromStr;

use assay_domain::{ContentHash, EvidenceId, RepositorySource, RevisionId};

pub const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
pub const TREE: &str = "89abcdef0123456789abcdef0123456789abcdef";
pub const SHA256: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
pub const OTHER_SHA256: &str =
    "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

pub fn local_source() -> RepositorySource {
    RepositorySource::local(ContentHash::from_str(SHA256).unwrap())
}

pub fn snapshot() -> assay_domain::SourceSnapshot {
    assay_domain::SourceSnapshot::new(
        local_source(),
        RevisionId::from_str(REVISION).unwrap(),
        Some(RevisionId::from_str(TREE).unwrap()),
    )
}

pub fn evidence(status: assay_domain::EvidenceStatus) -> assay_domain::EvidenceSource {
    assay_domain::EvidenceSource::at_revision(
        EvidenceId::from_str("evidence:repository:snapshot").unwrap(),
        assay_domain::EvidenceSourceKind::Repository,
        status,
        RevisionId::from_str(REVISION).unwrap(),
    )
}

mod manifests;
mod scalars;
