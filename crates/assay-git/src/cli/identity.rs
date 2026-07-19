use std::{ffi::OsStr, path::Path, str::FromStr};

use assay_domain::{ContentHash, RepositorySource, RevisionId};
use sha2::{Digest, Sha256};

use crate::{
    CollectionError, CollectionErrorKind, CollectionStage, GitObjectFormat,
    ResolvedLocalRepository, topology::RepositoryTopology,
};

use super::GitCliAdapter;
use super::error::repository_redirect;
use super::parse::parse_lines_of_object_ids;

impl GitCliAdapter {
    /// Derives a portable local repository identity from the sorted reachable
    /// root commits at the requested revision. Absolute paths are never an
    /// identity input. Shallow state is domain-separated because a boundary
    /// commit is not the full repository root.
    pub fn derive_local_repository_source(
        &self,
        repository: &Path,
        revision: &OsStr,
    ) -> Result<ResolvedLocalRepository, CollectionError> {
        let topology = RepositoryTopology::inspect(repository)?;
        self.validate_object_store(repository, &topology)?;
        let format = self.object_format(repository)?;
        let shallow = self.is_shallow(repository)?;
        let resolved = self.resolve_revision(repository, revision, format)?;
        let max_output = self
            .limits
            .max_history_commits
            .checked_mul(format.identifier_length() + 1)
            .ok_or_else(|| {
                CollectionError::new(
                    CollectionStage::DeriveRepositoryIdentity,
                    CollectionErrorKind::OutputLimit,
                )
            })?;
        let output = self.runner.run(
            Some(repository),
            CollectionStage::DeriveRepositoryIdentity,
            &[
                OsStr::new("rev-list"),
                OsStr::new("--max-parents=0"),
                OsStr::new("--topo-order"),
                OsStr::new("--end-of-options"),
                OsStr::new(resolved.as_str()),
            ],
            max_output,
        )?;
        let mut roots =
            parse_lines_of_object_ids(&output, CollectionStage::DeriveRepositoryIdentity, format)?;
        if roots.is_empty() || roots.len() > self.limits.max_history_commits {
            return Err(CollectionError::new(
                CollectionStage::DeriveRepositoryIdentity,
                CollectionErrorKind::RecordLimit,
            ));
        }
        roots.sort();
        if roots.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(CollectionError::new(
                CollectionStage::DeriveRepositoryIdentity,
                CollectionErrorKind::MalformedOutput,
            ));
        }
        let mut digest = Sha256::new();
        digest.update(b"assay-local-repository-id-v1\0");
        digest.update(match format {
            GitObjectFormat::Sha1 => b"sha1".as_slice(),
            GitObjectFormat::Sha256 => b"sha256".as_slice(),
        });
        digest.update([u8::from(shallow)]);
        for root in roots {
            let bytes = root.as_str().as_bytes();
            digest.update((bytes.len() as u64).to_be_bytes());
            digest.update(bytes);
        }
        let hash = ContentHash::from_str(&format!("sha256:{}", hex::encode(digest.finalize())))
            .map_err(|_| {
                CollectionError::new(
                    CollectionStage::DeriveRepositoryIdentity,
                    CollectionErrorKind::MalformedOutput,
                )
            })?;
        let final_topology = RepositoryTopology::inspect(repository)?;
        if final_topology != topology {
            return Err(repository_redirect());
        }
        self.validate_object_store(repository, &final_topology)?;
        if self.object_format(repository)? != format || self.is_shallow(repository)? != shallow {
            return Err(repository_redirect());
        }
        let revision = RevisionId::from_str(resolved.as_str()).map_err(|_| {
            CollectionError::new(
                CollectionStage::DeriveRepositoryIdentity,
                CollectionErrorKind::MalformedOutput,
            )
        })?;
        Ok(ResolvedLocalRepository::new(
            RepositorySource::local(hash),
            revision,
        ))
    }
}
