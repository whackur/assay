use assay_domain::{EvidenceId, EvidenceStatus};
use assay_git::ParentDeltaIssue;

use super::id::EvidenceIdBuilder;
use super::mapping::parent_delta_values;

#[test]
fn evidence_id_normalization_is_length_prefixed_and_kind_separated() {
    fn id(kind: &str, fields: &[(&[u8], &[u8])]) -> EvidenceId {
        let mut builder = EvidenceIdBuilder::new(kind);
        for (name, value) in fields {
            builder.field(name, value);
        }
        builder.finish(kind).unwrap()
    }

    let first = id("tracked-file", &[(b"a", b"bc"), (b"ab", b"c")]);
    let ambiguous_without_lengths = id("tracked-file", &[(b"a", b"bca"), (b"b", b"c")]);
    let different_kind = id("file-classification", &[(b"a", b"bc"), (b"ab", b"c")]);

    assert_ne!(first, ambiguous_without_lengths);
    assert_ne!(first, different_kind);
    assert_eq!(
        first.as_str(),
        "evidence:tracked-file:v1-5765416995d5c02f05916bbbb36f2deff800535712caf2397f098db769fc823b"
    );
}

#[test]
fn parent_delta_values_preserve_only_observed_counts() {
    assert_eq!(
        parent_delta_values(EvidenceStatus::Complete, None, 3, 1),
        (Some(3), Some(1))
    );
    assert_eq!(
        parent_delta_values(
            EvidenceStatus::Partial,
            Some(ParentDeltaIssue::RenameCandidateLimit),
            3,
            0,
        ),
        (Some(3), None)
    );
    assert_eq!(
        parent_delta_values(
            EvidenceStatus::Partial,
            Some(ParentDeltaIssue::ShallowRepository),
            0,
            0,
        ),
        (None, None)
    );
    assert_eq!(
        parent_delta_values(
            EvidenceStatus::Unavailable,
            Some(ParentDeltaIssue::ProcessFailure),
            0,
            0,
        ),
        (None, None)
    );
}
