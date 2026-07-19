use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::ContentHash;
use crate::error::{DomainValueError, is_safe_component};

fn validate_locator_component(value: &str) -> Result<(), &'static str> {
    if !is_safe_component(value) {
        return Err("expected a canonical portable repository component");
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct LocatorComponent(String);

impl TryFrom<String> for LocatorComponent {
    type Error = DomainValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_locator_component(&value)
            .map_err(|reason| DomainValueError::new("repository_source", reason))?;
        Ok(Self(value))
    }
}

impl Serialize for LocatorComponent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for LocatorComponent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

impl LocatorComponent {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum RepositorySourceData {
    Local {
        repository_id: ContentHash,
    },
    Hosted {
        provider: LocatorComponent,
        namespace: LocatorComponent,
        repository: LocatorComponent,
    },
}

/// A portable repository locator that cannot contain a local filesystem path.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RepositorySource(RepositorySourceData);

impl RepositorySource {
    /// Creates a local source identified by a content-derived identifier.
    pub const fn local(repository_id: ContentHash) -> Self {
        Self(RepositorySourceData::Local { repository_id })
    }

    /// Creates a canonical provider-neutral hosted repository locator.
    pub fn hosted(
        provider: &str,
        namespace: &str,
        repository: &str,
    ) -> Result<Self, DomainValueError> {
        Ok(Self(RepositorySourceData::Hosted {
            provider: LocatorComponent::try_from(provider.to_owned())?,
            namespace: LocatorComponent::try_from(namespace.to_owned())?,
            repository: LocatorComponent::try_from(repository.to_owned())?,
        }))
    }

    /// Returns the content-derived local repository identifier, when local.
    pub const fn local_repository_id(&self) -> Option<&ContentHash> {
        match &self.0 {
            RepositorySourceData::Local { repository_id } => Some(repository_id),
            RepositorySourceData::Hosted { .. } => None,
        }
    }

    /// Returns canonical provider, namespace, and repository components, when hosted.
    pub fn hosted_locator(&self) -> Option<(&str, &str, &str)> {
        match &self.0 {
            RepositorySourceData::Hosted {
                provider,
                namespace,
                repository,
            } => Some((provider.as_str(), namespace.as_str(), repository.as_str())),
            RepositorySourceData::Local { .. } => None,
        }
    }
}
