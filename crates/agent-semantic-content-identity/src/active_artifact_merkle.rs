//! Content identity for the active ASP artifact set and its Merkle receipt.

use serde::{Deserialize, Serialize};

use crate::exact_selector_merkle::{
    ContentDigestV1, canonical_content_digest, parse_content_digest_v1,
};

/// Stable schema identifier for an active ASP artifact receipt.
pub const ACTIVE_ASP_ARTIFACT_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.active-asp-artifact-receipt";
/// Stable schema version for an active ASP artifact receipt.
pub const ACTIVE_ASP_ARTIFACT_RECEIPT_SCHEMA_VERSION: &str = "1";
/// Digest algorithm used for artifact and materialization roots.
pub const ACTIVE_ASP_ARTIFACT_DIGEST_ALGORITHM: &str = "blake3-256";

/// Stable identity of one active artifact set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActiveArtifactSetId(String);

impl ActiveArtifactSetId {
    /// Create an artifact-set identity from its stable string value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Return the stable artifact-set identity string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ActiveArtifactSetId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ActiveArtifactSetId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// Semantic role of one leaf in the active artifact set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActiveArtifactKind {
    ExactSelectorGenerationFixture,
    ProviderRelationGeneration,
    AspBinary,
    Activation,
    ProviderBinary,
    ProviderRegistry,
    RuntimeConfig,
}

impl ActiveArtifactKind {
    /// Return the canonical receipt spelling for this artifact role.
    pub fn canonical_name(self) -> &'static str {
        match self {
            Self::AspBinary => "asp-binary",
            Self::Activation => "activation",
            Self::ProviderBinary => "provider-binary",
            Self::ProviderRegistry => "provider-registry",
            Self::RuntimeConfig => "runtime-config",
            Self::ExactSelectorGenerationFixture => "exact-selector-generation-fixture",
            Self::ProviderRelationGeneration => "provider-relation-generation",
        }
    }
}

/// Content and materialization identity of one active artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActiveArtifactLeaf {
    logical_path: ActiveArtifactLogicalPathV1,
    materialized_path: ActiveArtifactMaterializedPathV1,
    artifact_kind: ActiveArtifactKind,
    artifact_digest: ContentDigestV1,
    size_bytes: ActiveArtifactByteCountV1,
    #[serde(default)]
    modified_unix_nanos: ActiveArtifactModifiedUnixNanosV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    change_time_unix_nanos: Option<ActiveArtifactChangeTimeUnixNanosV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
struct ActiveArtifactLogicalPathV1(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
struct ActiveArtifactMaterializedPathV1(String);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
struct ActiveArtifactByteCountV1(u64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
struct ActiveArtifactModifiedUnixNanosV1(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
struct ActiveArtifactChangeTimeUnixNanosV1(i64);

/// Named construction input for one active artifact leaf.
pub struct ActiveArtifactLeafInput {
    logical_path: String,
    materialized_path: String,
    artifact_kind: ActiveArtifactKind,
    artifact_digest: ContentDigestV1,
    size_bytes: u64,
    modified_unix_nanos: u64,
    change_time_unix_nanos: Option<i64>,
}

impl ActiveArtifactLeafInput {
    /// Create the logical and content identity of one artifact leaf.
    pub fn new(
        logical_path: impl Into<String>,
        materialized_path: impl Into<String>,
        artifact_kind: ActiveArtifactKind,
        artifact_digest: ContentDigestV1,
    ) -> Self {
        Self {
            logical_path: logical_path.into(),
            materialized_path: materialized_path.into(),
            artifact_kind,
            artifact_digest,
            size_bytes: 0,
            modified_unix_nanos: 0,
            change_time_unix_nanos: None,
        }
    }

    /// Attach filesystem metadata without changing logical content identity.
    pub fn with_materialization_metadata(
        mut self,
        size_bytes: u64,
        modified_unix_nanos: u64,
        change_time_unix_nanos: Option<i64>,
    ) -> Self {
        self.size_bytes = size_bytes;
        self.modified_unix_nanos = modified_unix_nanos;
        self.change_time_unix_nanos = change_time_unix_nanos;
        self
    }
}

impl ActiveArtifactLeaf {
    /// Validate and construct one artifact leaf.
    pub fn new(input: ActiveArtifactLeafInput) -> Result<Self, String> {
        let ActiveArtifactLeafInput {
            logical_path,
            materialized_path,
            artifact_kind,
            artifact_digest,
            size_bytes,
            modified_unix_nanos,
            change_time_unix_nanos,
        } = input;
        if logical_path.is_empty() || materialized_path.is_empty() {
            return Err(
                "active artifact logical and materialized paths must be non-empty".to_string(),
            );
        }
        if change_time_unix_nanos.is_some_and(|value| value < 0) {
            return Err("active artifact change time must be non-negative".to_string());
        }
        Ok(Self {
            logical_path: ActiveArtifactLogicalPathV1(logical_path),
            materialized_path: ActiveArtifactMaterializedPathV1(materialized_path),
            artifact_kind,
            artifact_digest,
            size_bytes: ActiveArtifactByteCountV1(size_bytes),
            modified_unix_nanos: ActiveArtifactModifiedUnixNanosV1(modified_unix_nanos),
            change_time_unix_nanos: change_time_unix_nanos.map(ActiveArtifactChangeTimeUnixNanosV1),
        })
    }

    /// Return the stable logical path within the artifact set.
    pub fn logical_path(&self) -> &str {
        &self.logical_path.0
    }

    /// Return the concrete materialized path used by the active runtime.
    pub fn materialized_path(&self) -> &str {
        &self.materialized_path.0
    }

    /// Return the semantic artifact role.
    pub fn artifact_kind(&self) -> ActiveArtifactKind {
        self.artifact_kind
    }

    /// Return the content digest of the materialized artifact.
    pub fn artifact_digest(&self) -> &ContentDigestV1 {
        &self.artifact_digest
    }

    /// Return the artifact size in bytes.
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes.0
    }

    /// Return the observed modification time in Unix nanoseconds.
    pub fn modified_unix_nanos(&self) -> u64 {
        self.modified_unix_nanos.0
    }

    /// Return the optional change time in Unix nanoseconds.
    pub fn change_time_unix_nanos(&self) -> Option<i64> {
        self.change_time_unix_nanos.map(|value| value.0)
    }
}

/// Content-proven receipt for the complete active ASP artifact set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActiveAspArtifactReceipt {
    schema_id: String,
    schema_version: String,
    digest_algorithm: String,
    artifact_set_id: ActiveArtifactSetId,
    artifact_root_digest: ContentDigestV1,
    materialization_root_digest: ContentDigestV1,
    leaves: Vec<ActiveArtifactLeaf>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActiveAspArtifactReceiptWireV1 {
    schema_id: String,
    schema_version: String,
    digest_algorithm: String,
    artifact_set_id: ActiveArtifactSetId,
    artifact_root_digest: ContentDigestV1,
    #[serde(default)]
    materialization_root_digest: Option<ContentDigestV1>,
    leaves: Vec<ActiveArtifactLeaf>,
}

impl<'de> Deserialize<'de> for ActiveAspArtifactReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ActiveAspArtifactReceiptWireV1::deserialize(deserializer)?;
        let materialization_root_digest = match wire.materialization_root_digest {
            Some(digest) => digest,
            None => {
                active_artifact_materialization_root_digest_v1(&wire.artifact_set_id, &wire.leaves)
                    .map_err(|error| serde::de::Error::custom(format!("{error:?}")))?
            }
        };
        Ok(Self {
            schema_id: wire.schema_id,
            schema_version: wire.schema_version,
            digest_algorithm: wire.digest_algorithm,
            artifact_set_id: wire.artifact_set_id,
            artifact_root_digest: wire.artifact_root_digest,
            materialization_root_digest,
            leaves: wire.leaves,
        })
    }
}

/// Typed validation failures for active artifact receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveAspArtifactReceiptError {
    Identity,
    EmptyArtifactSetId,
    NonCanonicalPath(String),
    NonCanonicalMaterializedPath(String),
    NonCanonicalDigest(String),
    UnsortedOrDuplicateLeaves,
    AspBinaryLeafCount(usize),
    ActivationLeafCount(usize),
    RootDigestMismatch,
    MaterializationRootDigestMismatch,
}

impl ActiveAspArtifactReceipt {
    /// Return the logical artifact-set Merkle root.
    pub fn artifact_root_digest(&self) -> &ContentDigestV1 {
        &self.artifact_root_digest
    }

    /// Return the materialization-bound Merkle root.
    pub fn materialization_root_digest(&self) -> &ContentDigestV1 {
        &self.materialization_root_digest
    }

    /// Return the ordered artifact leaves covered by this receipt.
    pub fn leaves(&self) -> &[ActiveArtifactLeaf] {
        &self.leaves
    }

    /// Build and validate a deterministic receipt from an unordered leaf set.
    pub fn build(
        artifact_set_id: impl Into<String>,
        mut leaves: Vec<ActiveArtifactLeaf>,
    ) -> Result<Self, ActiveAspArtifactReceiptError> {
        let artifact_set_id = ActiveArtifactSetId::from(artifact_set_id.into());
        leaves.sort_by(|left, right| left.logical_path().cmp(right.logical_path()));
        let artifact_root_digest = active_artifact_root_digest_v1(&artifact_set_id, &leaves)?;
        let materialization_root_digest =
            active_artifact_materialization_root_digest_v1(&artifact_set_id, &leaves)?;
        let receipt = Self {
            schema_id: ACTIVE_ASP_ARTIFACT_RECEIPT_SCHEMA_ID.to_string(),
            schema_version: ACTIVE_ASP_ARTIFACT_RECEIPT_SCHEMA_VERSION.to_string(),
            digest_algorithm: ACTIVE_ASP_ARTIFACT_DIGEST_ALGORITHM.to_string(),
            artifact_set_id,
            artifact_root_digest,
            materialization_root_digest,
            leaves,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    /// Validate schema identity, leaf ordering, required roles, and both roots.
    pub fn validate(&self) -> Result<(), ActiveAspArtifactReceiptError> {
        if self.schema_id != ACTIVE_ASP_ARTIFACT_RECEIPT_SCHEMA_ID
            || self.schema_version != ACTIVE_ASP_ARTIFACT_RECEIPT_SCHEMA_VERSION
            || self.digest_algorithm != ACTIVE_ASP_ARTIFACT_DIGEST_ALGORITHM
        {
            return Err(ActiveAspArtifactReceiptError::Identity);
        }
        if self.artifact_set_id.as_str().is_empty() {
            return Err(ActiveAspArtifactReceiptError::EmptyArtifactSetId);
        }
        let mut previous_path: Option<&str> = None;
        let mut asp_binary_count = 0;
        let mut activation_count = 0;
        for leaf in &self.leaves {
            validate_logical_path(leaf.logical_path())?;
            parse_content_digest_v1(leaf.artifact_digest.as_str()).map_err(|_| {
                ActiveAspArtifactReceiptError::NonCanonicalDigest(leaf.logical_path().to_string())
            })?;
            if previous_path.is_some_and(|previous| previous >= leaf.logical_path()) {
                return Err(ActiveAspArtifactReceiptError::UnsortedOrDuplicateLeaves);
            }
            previous_path = Some(leaf.logical_path());
            asp_binary_count += usize::from(leaf.artifact_kind == ActiveArtifactKind::AspBinary);
            activation_count += usize::from(leaf.artifact_kind == ActiveArtifactKind::Activation);
        }
        if asp_binary_count != 1 {
            return Err(ActiveAspArtifactReceiptError::AspBinaryLeafCount(
                asp_binary_count,
            ));
        }
        if activation_count != 1 {
            return Err(ActiveAspArtifactReceiptError::ActivationLeafCount(
                activation_count,
            ));
        }
        if active_artifact_root_digest_v1(&self.artifact_set_id, &self.leaves)?
            != self.artifact_root_digest
        {
            return Err(ActiveAspArtifactReceiptError::RootDigestMismatch);
        }
        if active_artifact_materialization_root_digest_v1(&self.artifact_set_id, &self.leaves)?
            != self.materialization_root_digest
        {
            return Err(ActiveAspArtifactReceiptError::MaterializationRootDigestMismatch);
        }
        Ok(())
    }

    /// Return the unique validated ASP binary leaf.
    pub fn asp_binary_leaf(&self) -> &ActiveArtifactLeaf {
        self.leaves
            .iter()
            .find(|leaf| leaf.artifact_kind == ActiveArtifactKind::AspBinary)
            .expect("validated active ASP receipt has one binary leaf")
    }

    /// Return the unique validated activation leaf.
    pub fn activation_leaf(&self) -> &ActiveArtifactLeaf {
        self.leaves
            .iter()
            .find(|leaf| leaf.artifact_kind == ActiveArtifactKind::Activation)
            .expect("validated active ASP receipt has one activation leaf")
    }
}

/// Derive the logical content root for an active artifact set.
pub fn active_artifact_root_digest_v1(
    artifact_set_id: &ActiveArtifactSetId,
    leaves: &[ActiveArtifactLeaf],
) -> Result<ContentDigestV1, ActiveAspArtifactReceiptError> {
    if artifact_set_id.as_str().is_empty() {
        return Err(ActiveAspArtifactReceiptError::EmptyArtifactSetId);
    }
    let mut previous_path: Option<&str> = None;
    for leaf in leaves {
        validate_logical_path(leaf.logical_path())?;
        parse_content_digest_v1(leaf.artifact_digest.as_str()).map_err(|_| {
            ActiveAspArtifactReceiptError::NonCanonicalDigest(leaf.logical_path().to_string())
        })?;
        if previous_path.is_some_and(|previous| previous >= leaf.logical_path()) {
            return Err(ActiveAspArtifactReceiptError::UnsortedOrDuplicateLeaves);
        }
        previous_path = Some(leaf.logical_path());
    }
    let mut level = Vec::with_capacity(leaves.len());
    for leaf in leaves {
        validate_logical_path(leaf.logical_path())?;
        level.push(canonical_content_digest(
            b"asp.active-artifact-leaf.v1",
            &[
                leaf.logical_path().as_bytes(),
                leaf.artifact_kind.canonical_name().as_bytes(),
                leaf.artifact_digest.as_str().as_bytes(),
                &leaf.size_bytes().to_be_bytes(),
            ],
        ));
    }
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            if let [left, right] = pair {
                next.push(canonical_content_digest(
                    b"asp.active-artifact-node.v1",
                    &[left.as_str().as_bytes(), right.as_str().as_bytes()],
                ));
            } else {
                next.push(pair[0].clone());
            }
        }
        level = next;
    }
    let inner_root = level.first().map(ContentDigestV1::as_str).unwrap_or("");
    Ok(canonical_content_digest(
        b"asp.active-artifact-root.v1",
        &[
            artifact_set_id.as_str().as_bytes(),
            &(leaves.len() as u64).to_be_bytes(),
            inner_root.as_bytes(),
        ],
    ))
}

/// Derive the materialization-bound root for an active artifact set.
pub fn active_artifact_materialization_root_digest_v1(
    artifact_set_id: &ActiveArtifactSetId,
    leaves: &[ActiveArtifactLeaf],
) -> Result<ContentDigestV1, ActiveAspArtifactReceiptError> {
    if artifact_set_id.as_str().is_empty() {
        return Err(ActiveAspArtifactReceiptError::EmptyArtifactSetId);
    }
    let mut previous_path: Option<&str> = None;
    let mut level = Vec::with_capacity(leaves.len());
    for leaf in leaves {
        validate_logical_path(leaf.logical_path())?;
        validate_materialized_path(leaf.materialized_path())?;
        if previous_path.is_some_and(|previous| previous >= leaf.logical_path()) {
            return Err(ActiveAspArtifactReceiptError::UnsortedOrDuplicateLeaves);
        }
        previous_path = Some(leaf.logical_path());
        level.push(canonical_content_digest(
            b"asp.active-artifact-materialization-leaf.v1",
            &[
                leaf.logical_path().as_bytes(),
                leaf.materialized_path().as_bytes(),
                leaf.artifact_kind.canonical_name().as_bytes(),
                leaf.artifact_digest.as_str().as_bytes(),
                &leaf.size_bytes().to_be_bytes(),
                &leaf.modified_unix_nanos().to_be_bytes(),
                &leaf
                    .change_time_unix_nanos()
                    .unwrap_or_default()
                    .to_be_bytes(),
            ],
        ));
    }
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            if let [left, right] = pair {
                next.push(canonical_content_digest(
                    b"asp.active-artifact-materialization-node.v1",
                    &[left.as_str().as_bytes(), right.as_str().as_bytes()],
                ));
            } else {
                next.push(pair[0].clone());
            }
        }
        level = next;
    }
    let inner_root = level.first().map(ContentDigestV1::as_str).unwrap_or("");
    Ok(canonical_content_digest(
        b"asp.active-artifact-materialization-root.v1",
        &[
            artifact_set_id.as_str().as_bytes(),
            &(leaves.len() as u64).to_be_bytes(),
            inner_root.as_bytes(),
        ],
    ))
}

fn validate_materialized_path(path: &str) -> Result<(), ActiveAspArtifactReceiptError> {
    let materialized = std::path::Path::new(path);
    if path.is_empty()
        || !materialized.is_absolute()
        || materialized.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        return Err(ActiveAspArtifactReceiptError::NonCanonicalMaterializedPath(
            path.to_string(),
        ));
    }
    Ok(())
}

fn validate_logical_path(path: &str) -> Result<(), ActiveAspArtifactReceiptError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.split('/').any(|segment| {
            segment.is_empty() || segment == "." || segment == ".." || segment.contains('\\')
        })
    {
        return Err(ActiveAspArtifactReceiptError::NonCanonicalPath(
            path.to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/active_artifact_merkle_v1.rs"]
mod tests;
