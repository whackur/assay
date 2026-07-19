use serde_json::{Value, json};

use assay_domain::RepositorySource;

pub(crate) fn privacy() -> Value {
    json!({ "visibility": "private_local", "source_content": "not_retained", "external_transmission": "prohibited" })
}

pub(crate) fn map_repository(source: &RepositorySource) -> Value {
    if let Some(id) = source.local_repository_id() {
        json!({ "kind": "local", "repository_id": id.as_str() })
    } else if let Some((provider, namespace, repository)) = source.hosted_locator() {
        json!({ "kind": "hosted", "provider": provider, "namespace": namespace, "repository": repository })
    } else {
        unreachable!("repository source variants are closed")
    }
}

pub(crate) fn repository_identity_component(source: &RepositorySource) -> String {
    if let Some(id) = source.local_repository_id() {
        format!("local:{}", id.as_str())
    } else if let Some((provider, namespace, repository)) = source.hosted_locator() {
        format!("hosted:{provider}:{namespace}:{repository}")
    } else {
        unreachable!("repository source variants are closed")
    }
}
