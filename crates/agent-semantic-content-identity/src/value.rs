//! Validated value boundaries shared by artifact models and hashing operations.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::domain::HASH_ALGORITHM_BLAKE3;

/// Stable content digest used by ASP artifact roots, nodes, and leaves.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactHash {
    /// Digest algorithm token, currently always `blake3`.
    pub algorithm: String,
    /// Lowercase hex digest value.
    pub value: String,
}

impl ArtifactHash {
    /// Hash bytes with the canonical artifact digest algorithm.
    pub fn blake3(bytes: impl AsRef<[u8]>) -> Self {
        Self::from_blake3_output(blake3::hash(bytes.as_ref()))
    }

    pub(crate) fn from_blake3_output(output: blake3::Hash) -> Self {
        Self {
            algorithm: HASH_ALGORITHM_BLAKE3.to_string(),
            value: output.to_hex().to_string(),
        }
    }

    /// Render the hash as an integrity reference such as `blake3:<hex>`.
    pub fn as_integrity_ref(&self) -> String {
        format!("{}:{}", self.algorithm, self.value)
    }
}

impl fmt::Display for ArtifactHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.as_integrity_ref())
    }
}

/// Named artifact JSON boundary for canonical JSON hashing.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ArtifactJson {
    value: serde_json::Value,
}

impl ArtifactJson {
    /// Serialize a typed value into the canonical artifact JSON boundary.
    pub fn from_serializable<T: Serialize + ?Sized>(value: &T) -> Result<Self, serde_json::Error> {
        serde_json::to_value(value).map(|value| Self { value })
    }

    pub(crate) fn as_value(&self) -> &serde_json::Value {
        &self.value
    }
}

/// Stable State Core repository identity.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct ArtifactRepoId(String);

impl ArtifactRepoId {
    /// Create a repository identity.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the repository identity token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable State Core workspace identity.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct ArtifactWorkspaceId(String);

impl ArtifactWorkspaceId {
    /// Create a workspace identity.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the workspace identity token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable artifact scope identity.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct ArtifactScopeId(String);

impl ArtifactScopeId {
    /// Create an artifact scope identity.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the scope identity token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable artifact generation identity.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct ArtifactGeneration(String);

impl ArtifactGeneration {
    /// Create an artifact generation identity.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the generation identity token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Artifact node or root kind.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct ArtifactKind(String);

impl ArtifactKind {
    /// Create an artifact kind token.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the artifact kind token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
