use serde::{Deserialize, Serialize};

use crate::exact_selector_merkle::{
    ContentDigestV1, canonical_content_digest_v1, parse_content_digest_v1,
};

pub const ACTIVE_ASP_ARTIFACT_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.active-asp-artifact-receipt";
pub const ACTIVE_ASP_ARTIFACT_RECEIPT_SCHEMA_VERSION: &str = "1";
pub const ACTIVE_ASP_ARTIFACT_DIGEST_ALGORITHM: &str = "blake3-256";

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
    pub logical_path: String,
    pub materialized_path: String,
    pub artifact_kind: ActiveArtifactKindV1,
    pub artifact_digest: ContentDigestV1,
    pub size_bytes: u64,
    #[serde(default)]
    pub modified_unix_nanos: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_time_unix_nanos: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActiveAspArtifactReceiptV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub digest_algorithm: String,
    pub artifact_set_id: String,
    pub artifact_root_digest: ContentDigestV1,
    pub leaves: Vec<ActiveArtifactLeafV1>,
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
}

impl ActiveAspArtifactReceiptV1 {
    pub fn build(
        artifact_set_id: impl Into<String>,
        mut leaves: Vec<ActiveArtifactLeafV1>,
    ) -> Result<Self, ActiveAspArtifactReceiptV1Error> {
        let artifact_set_id = artifact_set_id.into();
        leaves.sort_by(|left, right| left.logical_path.cmp(&right.logical_path));
        let artifact_root_digest = active_artifact_root_digest_v1(&artifact_set_id, &leaves)?;
        let receipt = Self {
            schema_id: ACTIVE_ASP_ARTIFACT_RECEIPT_SCHEMA_ID.to_string(),
            schema_version: ACTIVE_ASP_ARTIFACT_RECEIPT_SCHEMA_VERSION.to_string(),
            digest_algorithm: ACTIVE_ASP_ARTIFACT_DIGEST_ALGORITHM.to_string(),
            artifact_set_id,
            artifact_root_digest,
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
        if self.artifact_set_id.is_empty() {
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
    artifact_set_id: &str,
    leaves: &[ActiveArtifactLeafV1],
) -> Result<ContentDigestV1, ActiveAspArtifactReceiptV1Error> {
    if artifact_set_id.is_empty() {
        return Err(ActiveAspArtifactReceiptV1Error::EmptyArtifactSetId);
    }
    let mut previous_path: Option<&str> = None;
    for leaf in leaves {
        validate_logical_path(&leaf.logical_path)?;
        validate_materialized_path(&leaf.materialized_path)?;
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
        validate_materialized_path(&leaf.materialized_path)?;
        level.push(canonical_content_digest_v1(
            b"asp.active-artifact-leaf.v1",
            &[
                leaf.logical_path.as_bytes(),
                leaf.materialized_path.as_bytes(),
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
            artifact_set_id.as_bytes(),
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
mod tests {
    use super::{
        ActiveArtifactKindV1, ActiveArtifactLeafV1, ActiveAspArtifactReceiptV1,
        ActiveAspArtifactReceiptV1Error,
    };
    use crate::exact_selector_merkle::blake3_content_digest_v1;

    fn leaf(
        logical_path: &str,
        artifact_kind: ActiveArtifactKindV1,
        bytes: &[u8],
    ) -> ActiveArtifactLeafV1 {
        ActiveArtifactLeafV1 {
            logical_path: logical_path.to_string(),
            materialized_path: format!("/active/{logical_path}"),
            artifact_kind,
            artifact_digest: blake3_content_digest_v1(bytes),
            size_bytes: bytes.len() as u64,
        }
    }

    #[test]
    fn receipt_is_sorted_and_binds_every_leaf() {
        let receipt = ActiveAspArtifactReceiptV1::build(
            "asp-runtime",
            vec![
                leaf(
                    "state/activation.json",
                    ActiveArtifactKindV1::Activation,
                    b"activation",
                ),
                leaf(
                    "runtime/bin/by-digest/abc/asp",
                    ActiveArtifactKindV1::AspBinary,
                    b"asp",
                ),
            ],
        )
        .expect("active artifact receipt");
        assert_eq!(receipt.schema_version, "1");
        assert_eq!(receipt.asp_binary_leaf().size_bytes, 3);
        assert_eq!(receipt.activation_leaf().size_bytes, 10);

        let mut changed = receipt.clone();
        changed.leaves[0].size_bytes += 1;
        assert_eq!(
            changed.validate(),
            Err(ActiveAspArtifactReceiptV1Error::RootDigestMismatch)
        );
    }

    #[test]
    fn receipt_rejects_duplicate_or_missing_required_leaves() {
        let binary = leaf(
            "runtime/bin/by-digest/abc/asp",
            ActiveArtifactKindV1::AspBinary,
            b"asp",
        );
        assert!(matches!(
            ActiveAspArtifactReceiptV1::build("asp-runtime", vec![binary]),
            Err(ActiveAspArtifactReceiptV1Error::ActivationLeafCount(0))
        ));
    }
}
