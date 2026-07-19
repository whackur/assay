mod entries;
mod history;

use std::{ffi::OsStr, path::Path, str::FromStr};

use assay_domain::{ContentHash, EvidenceStatus};
use sha2::{Digest, Sha256};

use crate::{CollectionStage, GitObjectId, ObjectIssue, ObjectMetadata};

use super::GitCliAdapter;
use super::error::object_issue;
use super::parse::parse_decimal;

impl GitCliAdapter {
    pub(crate) fn collect_object_metadata(
        &self,
        repository: &Path,
        object_id: &GitObjectId,
    ) -> ObjectMetadata {
        let size_output = match self.runner.run(
            Some(repository),
            CollectionStage::ReadObjectMetadata,
            &[
                OsStr::new("cat-file"),
                OsStr::new("-s"),
                OsStr::new(object_id.as_str()),
            ],
            64,
        ) {
            Ok(output) => output,
            Err(error) => {
                return ObjectMetadata::unresolved(
                    EvidenceStatus::Unavailable,
                    object_issue(error.kind()),
                );
            }
        };
        let size = match parse_decimal(&size_output, CollectionStage::ReadObjectMetadata) {
            Ok(size) => size,
            Err(_) => {
                return ObjectMetadata::unresolved(
                    EvidenceStatus::Unavailable,
                    ObjectIssue::MalformedMetadata,
                );
            }
        };
        if size > self.limits.max_object_bytes {
            return ObjectMetadata::limited(size);
        }
        let stdout_limit = match usize::try_from(self.limits.max_object_bytes) {
            Ok(limit) => limit,
            Err(_) => self.limits.max_stdout_bytes,
        };
        let bytes = match self.runner.run(
            Some(repository),
            CollectionStage::HashObject,
            &[
                OsStr::new("cat-file"),
                OsStr::new("blob"),
                OsStr::new(object_id.as_str()),
            ],
            stdout_limit,
        ) {
            Ok(bytes) => bytes,
            Err(error) => {
                return ObjectMetadata::unresolved(
                    EvidenceStatus::Unavailable,
                    object_issue(error.kind()),
                );
            }
        };
        if u64::try_from(bytes.len()).ok() != Some(size) {
            return ObjectMetadata::unresolved(
                EvidenceStatus::Unavailable,
                ObjectIssue::MalformedMetadata,
            );
        }
        let digest = Sha256::digest(&bytes);
        let mut encoded = String::with_capacity(71);
        encoded.push_str("sha256:");
        for byte in digest {
            use std::fmt::Write;
            write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
        }
        let content_hash = ContentHash::from_str(&encoded)
            .expect("SHA-256 output always satisfies the domain digest invariant");
        ObjectMetadata::complete(size, content_hash)
    }
}
