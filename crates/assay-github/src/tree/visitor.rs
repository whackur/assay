use std::{fmt, str::FromStr};

use serde::de;

use crate::{BlobCacheLookup, GitHubObjectId, tree::contract::TreeSink};

use super::handler::TreeHandler;

pub(crate) struct TreeEnvelope {
    pub(crate) truncated: bool,
}

pub(crate) struct TreeEnvelopeSeed<'handler, 'context, C, S> {
    pub(crate) handler: &'handler mut TreeHandler<'context, C, S>,
}

impl<'de, C: BlobCacheLookup, S: TreeSink> de::DeserializeSeed<'de>
    for TreeEnvelopeSeed<'_, '_, C, S>
{
    type Value = TreeEnvelope;

    fn deserialize<D: de::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_map(TreeEnvelopeVisitor {
            handler: self.handler,
        })
    }
}

struct TreeEnvelopeVisitor<'handler, 'context, C, S> {
    handler: &'handler mut TreeHandler<'context, C, S>,
}

impl<'de, C: BlobCacheLookup, S: TreeSink> de::Visitor<'de> for TreeEnvelopeVisitor<'_, '_, C, S> {
    type Value = TreeEnvelope;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded GitHub tree response")
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut sha = None;
        let mut truncated = None;
        let mut tree_seen = false;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "sha" if sha.is_none() => {
                    let value = map.next_value::<String>()?;
                    GitHubObjectId::from_str(&value)
                        .map_err(|_| de::Error::custom("github_tree_invalid_root_id"))?;
                    sha = Some(value);
                }
                "truncated" if truncated.is_none() => truncated = Some(map.next_value()?),
                "tree" if !tree_seen => {
                    map.next_value_seed(TreeEntriesSeed {
                        handler: self.handler,
                    })?;
                    tree_seen = true;
                }
                "sha" | "truncated" | "tree" => {
                    return Err(de::Error::custom("github_tree_duplicate_field"));
                }
                _ => {
                    map.next_value::<de::IgnoredAny>()?;
                }
            }
        }
        if sha.is_none() || !tree_seen {
            return Err(de::Error::custom("github_tree_missing_field"));
        }
        Ok(TreeEnvelope {
            truncated: truncated.ok_or_else(|| de::Error::custom("github_tree_missing_field"))?,
        })
    }
}

struct TreeEntriesSeed<'handler, 'context, C, S> {
    handler: &'handler mut TreeHandler<'context, C, S>,
}

impl<'de, C: BlobCacheLookup, S: TreeSink> de::DeserializeSeed<'de>
    for TreeEntriesSeed<'_, '_, C, S>
{
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(TreeEntriesVisitor {
            handler: self.handler,
        })
    }
}

struct TreeEntriesVisitor<'handler, 'context, C, S> {
    handler: &'handler mut TreeHandler<'context, C, S>,
}

impl<'de, C: BlobCacheLookup, S: TreeSink> de::Visitor<'de> for TreeEntriesVisitor<'_, '_, C, S> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a sequence of GitHub tree entries")
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        while let Some(entry) = sequence.next_element::<super::handler::TreeEntry>()? {
            let _mode_is_present = !entry.mode.is_empty();
            self.handler.handle(entry)?;
        }
        Ok(())
    }
}
