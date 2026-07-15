use std::collections::BTreeSet;

use serde_json::Value;

pub(crate) const REPOSITORY_FEATURE_NAMES: [&str; 11] = [
    "readme",
    "license",
    "package_manifest",
    "ci",
    "test",
    "documentation",
    "migration",
    "dependency",
    "security_policy",
    "generated_content",
    "vendored_content",
];

pub(crate) struct RepositoryFeatureExpectation {
    pub(crate) state: &'static str,
    pub(crate) related_evidence_ids: Vec<String>,
}

/// Derives one feature solely from records in the public evidence bundle.
///
/// A payload-free `path_length_limit` envelope deliberately hides the path, so
/// this boundary cannot reproduce whether that record was a direct name or
/// category match. The conservative policy is to treat every opaque envelope
/// in the applicable evidence layer as a global uncertainty cause whenever no
/// reliable public match exists. A reliable match takes precedence and cites
/// only the matching factual records. An opaque cause means that absence cannot
/// be established; it does not claim that the hidden record contains the
/// feature.
pub(crate) fn derive_repository_feature<'a>(
    feature: &str,
    evidence: impl IntoIterator<Item = &'a Value>,
) -> Result<RepositoryFeatureExpectation, &'static str> {
    let specification = FeatureSpecification::from_name(feature).ok_or("feature_name")?;
    let mut reliable = BTreeSet::new();
    let mut direct_candidates = BTreeSet::new();
    let mut uncertainty_causes = BTreeSet::new();

    for fact in evidence {
        match specification {
            FeatureSpecification::Path(path_feature) => {
                collect_path_evidence(fact, path_feature, &mut reliable, &mut uncertainty_causes)
            }
            FeatureSpecification::Classification(category) => collect_classification_evidence(
                fact,
                category,
                &mut reliable,
                &mut direct_candidates,
                &mut uncertainty_causes,
            ),
        }
    }

    let (state, related) = if !reliable.is_empty() {
        ("present", reliable)
    } else if !direct_candidates.is_empty() {
        ("unavailable", direct_candidates)
    } else if !uncertainty_causes.is_empty() {
        ("unavailable", uncertainty_causes)
    } else {
        ("absent", BTreeSet::new())
    };
    Ok(RepositoryFeatureExpectation {
        state,
        related_evidence_ids: related.into_iter().collect(),
    })
}

#[derive(Clone, Copy)]
enum FeatureSpecification {
    Path(PathFeature),
    Classification(&'static str),
}

impl FeatureSpecification {
    fn from_name(feature: &str) -> Option<Self> {
        match feature {
            "readme" => Some(Self::Path(PathFeature::Readme)),
            "license" => Some(Self::Path(PathFeature::License)),
            "package_manifest" => Some(Self::Path(PathFeature::PackageManifest)),
            "ci" => Some(Self::Classification("ci_cd")),
            "test" => Some(Self::Classification("test")),
            "documentation" => Some(Self::Classification("documentation")),
            "migration" => Some(Self::Classification("schema_migration")),
            "dependency" => Some(Self::Classification("dependency")),
            "security_policy" => Some(Self::Classification("security")),
            "generated_content" => Some(Self::Classification("generated")),
            "vendored_content" => Some(Self::Classification("vendored")),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum PathFeature {
    Readme,
    License,
    PackageManifest,
}

impl PathFeature {
    fn matches(self, path: &str) -> bool {
        let lower = path.to_ascii_lowercase();
        let name = lower.rsplit('/').next().unwrap_or_default();
        match self {
            Self::Readme => name.starts_with("readme"),
            Self::License => name.starts_with("license") || name == "copying",
            Self::PackageManifest => matches!(
                name,
                "package.json" | "pyproject.toml" | "setup.py" | "setup.cfg"
            ),
        }
    }
}

fn collect_path_evidence(
    fact: &Value,
    feature: PathFeature,
    reliable: &mut BTreeSet<String>,
    uncertainty_causes: &mut BTreeSet<String>,
) {
    if evidence_kind(fact) != Some("tracked_file") {
        return;
    }
    let Some(id) = fact["id"].as_str() else {
        return;
    };
    let Some(payload) = fact.get("payload") else {
        uncertainty_causes.insert(id.to_owned());
        return;
    };
    let path = &payload["path"];
    if path["encoding"] != "utf8" {
        uncertainty_causes.insert(id.to_owned());
    } else if path["value"]
        .as_str()
        .is_some_and(|value| feature.matches(value))
    {
        reliable.insert(id.to_owned());
    }
}

fn collect_classification_evidence(
    fact: &Value,
    category: &str,
    reliable: &mut BTreeSet<String>,
    direct_candidates: &mut BTreeSet<String>,
    uncertainty_causes: &mut BTreeSet<String>,
) {
    if evidence_kind(fact) != Some("file_classification") {
        return;
    }
    let Some(id) = fact["id"].as_str() else {
        return;
    };
    let complete = fact["status"] == "complete";
    if !complete {
        uncertainty_causes.insert(id.to_owned());
    }
    if fact["payload"]["classification"]["primary_category"] == category {
        if complete {
            reliable.insert(id.to_owned());
        } else {
            direct_candidates.insert(id.to_owned());
        }
    }
}

fn evidence_kind(fact: &Value) -> Option<&str> {
    fact.get("payload")
        .and_then(|payload| payload["kind"].as_str())
        .or_else(|| fact["requested_kind"].as_str())
}

#[cfg(test)]
mod tests {
    use super::derive_repository_feature;
    use serde_json::{Value, json};

    #[test]
    fn path_features_cite_every_payload_free_raw_envelope_in_sorted_order() {
        // One hidden path is a direct-looking match and one is unrelated, but
        // the public payload-free envelopes are intentionally indistinguishable.
        let evidence = [
            envelope("evidence:tracked-file:v1-z", "tracked_file"),
            envelope("evidence:tracked-file:v1-a", "tracked_file"),
        ];

        let feature = derive_repository_feature("license", evidence.iter()).unwrap();

        assert_eq!(feature.state, "unavailable");
        assert_eq!(
            feature.related_evidence_ids,
            [
                "evidence:tracked-file:v1-a".to_owned(),
                "evidence:tracked-file:v1-z".to_owned(),
            ]
        );
    }

    #[test]
    fn classification_features_cite_every_payload_free_classification_envelope() {
        // Direct category provenance is also unavailable after payload removal.
        let evidence = [
            envelope("evidence:file-classification:v1-z", "file_classification"),
            envelope("evidence:file-classification:v1-a", "file_classification"),
        ];

        let feature = derive_repository_feature("test", evidence.iter()).unwrap();

        assert_eq!(feature.state, "unavailable");
        assert_eq!(
            feature.related_evidence_ids,
            [
                "evidence:file-classification:v1-a".to_owned(),
                "evidence:file-classification:v1-z".to_owned(),
            ]
        );
    }

    #[test]
    fn reliable_public_matches_take_precedence_over_opaque_causes() {
        let path_evidence = [
            envelope("evidence:tracked-file:v1-opaque", "tracked_file"),
            tracked_file("evidence:tracked-file:v1-readme", "README.md"),
        ];
        let classification_evidence = [
            envelope(
                "evidence:file-classification:v1-opaque",
                "file_classification",
            ),
            classification("evidence:file-classification:v1-test", "test"),
        ];

        let readme = derive_repository_feature("readme", path_evidence.iter()).unwrap();
        let test = derive_repository_feature("test", classification_evidence.iter()).unwrap();

        assert_eq!(readme.state, "present");
        assert_eq!(
            readme.related_evidence_ids,
            ["evidence:tracked-file:v1-readme".to_owned()]
        );
        assert_eq!(test.state, "present");
        assert_eq!(
            test.related_evidence_ids,
            ["evidence:file-classification:v1-test".to_owned()]
        );
    }

    fn envelope(id: &str, requested_kind: &str) -> Value {
        json!({
            "id": id,
            "status": "unsupported",
            "requested_kind": requested_kind,
            "reason": "path_length_limit"
        })
    }

    fn tracked_file(id: &str, path: &str) -> Value {
        json!({
            "id": id,
            "status": "complete",
            "payload": {
                "kind": "tracked_file",
                "path": { "encoding": "utf8", "value": path }
            }
        })
    }

    fn classification(id: &str, category: &str) -> Value {
        json!({
            "id": id,
            "status": "complete",
            "payload": {
                "kind": "file_classification",
                "classification": { "primary_category": category }
            }
        })
    }
}
