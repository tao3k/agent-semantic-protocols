// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::fmt;
use std::str::FromStr;

use agent_semantic_content_identity::exact_selector_merkle::ContentDigestV1;
use agent_semantic_content_identity::exact_selector_merkle::parse_content_digest_v1;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;

const PREFIX: &str = "blake3-256:";

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Blake3ContentDigest {
    canonical: String,
    content: ContentDigestV1,
}

impl Blake3ContentDigest {
    #[track_caller]
    pub fn parse(value: &str) -> Result<Self, String> {
        let caller = std::panic::Location::caller();
        Self::parse_inner(value).map_err(|error| {
            format!(
                "owner=Blake3ContentDigest field=contentDigest producer={}:{} {error}",
                caller.file(),
                caller.line()
            )
        })
    }

    fn parse_inner(value: &str) -> Result<Self, String> {
        let raw = value
            .strip_prefix(PREFIX)
            .ok_or_else(|| format!("BLAKE3 content digest must start with `{PREFIX}`"))?;
        let content = parse_content_digest_v1(raw)?;
        Ok(Self {
            canonical: format!("{PREFIX}{}", content.as_str()),
            content,
        })
    }

    pub fn from_content_digest(content: ContentDigestV1) -> Self {
        Self {
            canonical: format!("{PREFIX}{}", content.as_str()),
            content,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self::from_content_digest(
            agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(bytes),
        )
    }

    pub fn as_str(&self) -> &str {
        &self.canonical
    }

    pub fn content_digest(&self) -> &ContentDigestV1 {
        &self.content
    }

    pub fn into_content_digest(self) -> ContentDigestV1 {
        self.content
    }
}

impl fmt::Display for Blake3ContentDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Blake3ContentDigest {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Blake3ContentDigest {
    /// Reconstructs the strong digest identity from the lowercase hexadecimal
    /// component used by the immutable artifact directory layout.
    pub fn from_artifact_path_component(component: &str) -> Result<Self, String> {
        Self::parse(&format!("blake3-256:{component}"))
    }
}

impl Serialize for Blake3ContentDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Blake3ContentDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        crate::schema_v1_digest::canonicalize_schema_v1_blake3_value(value)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
#[path = "../tests/unit/blake3_content_digest.rs"]
mod tests;
