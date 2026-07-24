use serde::{Deserialize, Serialize};

use crate::exact_selector_merkle::{
    ContentDigestV1, canonical_content_digest_v1, parse_content_digest_v1,
};

pub const ACTIVE_ASP_ARTIFACT_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.active-asp-artifact-receipt";
pub const ACTIVE_ASP_ARTIFACT_RECEIPT_SCHEMA_VERSION: &str = "1";
pub const ACTIVE_ASP_ARTIFACT_DIGEST_ALGORITHM: &str = "blake3-256";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActiveArtifactSetIdV1(String);

impl ActiveArtifactSetIdV1 {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ActiveArtifactSetIdV1 {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ActiveArtifactSetIdV1 {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActiveArtifactKindV1 {
    AspBinary,
    Activation,
    ProviderBinary,
    ProviderRegistry,
    RuntimeConfig,
}

impl ActiveArtifactKindV1 {
    pub fn canonical_name(self) -> &'static str {
        match self {
            Self::AspBinary => "asp-binary",
            Self::Activation => "activation",
            Self::ProviderBinary => "provider-binary",
            Self::ProviderRegistry => "provider-registry",
            Self::RuntimeConfig => "runtime-config",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActiveArtifactLeafV1 {
    logical_path: String,
    materialized_path: String,
    artifact_kind: ActiveArtifactKindV1,
    artifact_digest: ContentDigestV1,
    size_bytes: u64,
    #[serde(default)]
    modified_unix_nanos: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    change_time_unix_nanos: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActiveAspArtifactReceiptV1 {
    schema_id: String,
    schema_version: String,
    digest_algorithm: String,
    artifact_set_id: ActiveArtifactSetIdV1,
    artifact_root_digest: ContentDigestV1,
    materialization_root_digest: ContentDigestV1,
    leaves: Vec<ActiveArtifactLeafV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveAspArtifactReceiptV1Error {
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

impl ActiveAspArtifactReceiptV1 {
    pub fn build(
        artifact_set_id: impl Into<String>,
        mut leaves: Vec<ActiveArtifactLeafV1>,
    ) -> Result<Self, ActiveAspArtifactReceiptV1Error> {
        let artifact_set_id = ActiveArtifactSetIdV1::from(artifact_set_id.into());
        leaves.sort_by(|left, right| left.logical_path.cmp(&right.logical_path));
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

    pub fn validate(&self) -> Result<(), ActiveAspArtifactReceiptV1Error> {
        if self.schema_id != ACTIVE_ASP_ARTIFACT_RECEIPT_SCHEMA_ID
            || self.schema_version != ACTIVE_ASP_ARTIFACT_RECEIPT_SCHEMA_VERSION
            || self.digest_algorithm != ACTIVE_ASP_ARTIFACT_DIGEST_ALGORITHM
        {
            return Err(ActiveAspArtifactReceiptV1Error::Identity);
        }
        if self.artifact_set_id.as_str().is_empty() {
            return Err(ActiveAspArtifactReceiptV1Error::EmptyArtifactSetId);
        }
        let mut previous_path: Option<&str> = None;
        let mut asp_binary_count = 0;
        let mut activation_count = 0;
        for leaf in &self.leaves {
            validate_logical_path(&leaf.logical_path)?;
            parse_content_digest_v1(leaf.artifact_digest.as_str()).map_err(|_| {
                ActiveAspArtifactReceiptV1Error::NonCanonicalDigest(leaf.logical_path.clone())
            })?;
            if previous_path.is_some_and(|previous| previous >= leaf.logical_path.as_str()) {
                return Err(ActiveAspArtifactReceiptV1Error::UnsortedOrDuplicateLeaves);
            }
            previous_path = Some(&leaf.logical_path);
            asp_binary_count += usize::from(leaf.artifact_kind == ActiveArtifactKindV1::AspBinary);
            activation_count += usize::from(leaf.artifact_kind == ActiveArtifactKindV1::Activation);
        }
        if asp_binary_count != 1 {
            return Err(ActiveAspArtifactReceiptV1Error::AspBinaryLeafCount(
                asp_binary_count,
            ));
        }
        if activation_count != 1 {
            return Err(ActiveAspArtifactReceiptV1Error::ActivationLeafCount(
                activation_count,
            ));
        }
        if active_artifact_root_digest_v1(&self.artifact_set_id, &self.leaves)?
            != self.artifact_root_digest
        {
            return Err(ActiveAspArtifactReceiptV1Error::RootDigestMismatch);
        }
        if active_artifact_materialization_root_digest_v1(&self.artifact_set_id, &self.leaves)?
            != self.materialization_root_digest
        {
            return Err(ActiveAspArtifactReceiptV1Error::MaterializationRootDigestMismatch);
        }
        Ok(())
    }

    pub fn asp_binary_leaf(&self) -> &ActiveArtifactLeafV1 {
        self.leaves
            .iter()
            .find(|leaf| leaf.artifact_kind == ActiveArtifactKindV1::AspBinary)
            .expect("validated active ASP receipt has one binary leaf")
    }

    pub fn activation_leaf(&self) -> &ActiveArtifactLeafV1 {
        self.leaves
            .iter()
            .find(|leaf| leaf.artifact_kind == ActiveArtifactKindV1::Activation)
            .expect("validated active ASP receipt has one activation leaf")
    }
}

pub fn active_artifact_root_digest_v1(
    artifact_set_id: &ActiveArtifactSetIdV1,
    leaves: &[ActiveArtifactLeafV1],
) -> Result<ContentDigestV1, ActiveAspArtifactReceiptV1Error> {
    if artifact_set_id.as_str().is_empty() {
        return Err(ActiveAspArtifactReceiptV1Error::EmptyArtifactSetId);
    }
    let mut previous_path: Option<&str> = None;
    for leaf in leaves {
        validate_logical_path(&leaf.logical_path)?;
        parse_content_digest_v1(leaf.artifact_digest.as_str()).map_err(|_| {
            ActiveAspArtifactReceiptV1Error::NonCanonicalDigest(leaf.logical_path.clone())
        })?;
        if previous_path.is_some_and(|previous| previous >= leaf.logical_path.as_str()) {
            return Err(ActiveAspArtifactReceiptV1Error::UnsortedOrDuplicateLeaves);
        }
        previous_path = Some(&leaf.logical_path);
    }
    let mut level = Vec::with_capacity(leaves.len());
    for leaf in leaves {
        validate_logical_path(&leaf.logical_path)?;
        level.push(canonical_content_digest_v1(
            b"asp.active-artifact-leaf.v1",
            &[
                leaf.logical_path.as_bytes(),
                leaf.artifact_kind.canonical_name().as_bytes(),
                leaf.artifact_digest.as_str().as_bytes(),
                &leaf.size_bytes.to_be_bytes(),
            ],
        ));
    }
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            if let [left, right] = pair {
                next.push(canonical_content_digest_v1(
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
    Ok(canonical_content_digest_v1(
        b"asp.active-artifact-root.v1",
        &[
            artifact_set_id.as_str().as_bytes(),
            &(leaves.len() as u64).to_be_bytes(),
            inner_root.as_bytes(),
        ],
    ))
}

pub fn active_artifact_materialization_root_digest_v1(
    artifact_set_id: &ActiveArtifactSetIdV1,
    leaves: &[ActiveArtifactLeafV1],
) -> Result<ContentDigestV1, ActiveAspArtifactReceiptV1Error> {
    if artifact_set_id.as_str().is_empty() {
        return Err(ActiveAspArtifactReceiptV1Error::EmptyArtifactSetId);
    }
    let mut previous_path: Option<&str> = None;
    let mut level = Vec::with_capacity(leaves.len());
    for leaf in leaves {
        validate_logical_path(&leaf.logical_path)?;
        validate_materialized_path(&leaf.materialized_path)?;
        if previous_path.is_some_and(|previous| previous >= leaf.logical_path.as_str()) {
            return Err(ActiveAspArtifactReceiptV1Error::UnsortedOrDuplicateLeaves);
        }
        previous_path = Some(&leaf.logical_path);
        level.push(canonical_content_digest_v1(
            b"asp.active-artifact-materialization-leaf.v1",
            &[
                leaf.logical_path.as_bytes(),
                leaf.materialized_path.as_bytes(),
                leaf.artifact_kind.canonical_name().as_bytes(),
                leaf.artifact_digest.as_str().as_bytes(),
                &leaf.size_bytes.to_be_bytes(),
                &leaf.modified_unix_nanos.to_be_bytes(),
                &leaf
                    .change_time_unix_nanos
                    .unwrap_or_default()
                    .to_be_bytes(),
            ],
        ));
    }
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            if let [left, right] = pair {
                next.push(canonical_content_digest_v1(
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
    Ok(canonical_content_digest_v1(
        b"asp.active-artifact-materialization-root.v1",
        &[
            artifact_set_id.as_str().as_bytes(),
            &(leaves.len() as u64).to_be_bytes(),
            inner_root.as_bytes(),
        ],
    ))
}

fn validate_materialized_path(path: &str) -> Result<(), ActiveAspArtifactReceiptV1Error> {
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
        return Err(
            ActiveAspArtifactReceiptV1Error::NonCanonicalMaterializedPath(path.to_string()),
        );
    }
    Ok(())
}

fn validate_logical_path(path: &str) -> Result<(), ActiveAspArtifactReceiptV1Error> {
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.split('/').any(|segment| {
            segment.is_empty() || segment == "." || segment == ".." || segment.contains('\\')
        })
    {
        return Err(ActiveAspArtifactReceiptV1Error::NonCanonicalPath(
            path.to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/active_artifact_merkle_v1.rs"]
mod tests;
