use serde::{Deserialize, Serialize};
use std::fmt;

pub const EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_ID: &str =
    "agent.semantic-protocols.exact-selector-merkle-proof";
pub const EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_VERSION: &str = "1";
pub const EXACT_SELECTOR_MERKLE_DIGEST_ALGORITHM: &str = "blake3-256";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExactSelectorMerkleProofV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub digest_algorithm: String,
    pub language_id: String,
    pub workspace_root_digest: ContentDigestV1,
    pub owner_path: String,
    pub owner_subtree_digest: ContentDigestV1,
    pub owner_inclusion_proof: Vec<MerkleInclusionStepV1>,
    pub source_blob_digest: ContentDigestV1,
    pub parser_identity_digest: ContentDigestV1,
    pub query_pack_digest: ContentDigestV1,
    pub parser_fact_digest: ContentDigestV1,
    pub canonical_item_selector: crate::canonical_item_identity::CanonicalItemSelectorV1,
    pub structural_selector: String,
    pub projection_mode: ExactProjectionModeV1,
    pub projection_digest: ContentDigestV1,
}

impl ExactSelectorMerkleProofV1 {
    pub fn validate_shape(&self) -> Result<(), ExactSelectorMerkleProofError> {
        if self.schema_id != EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_ID {
            return Err(ExactSelectorMerkleProofError::SchemaId);
        }
        if self.schema_version != EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_VERSION {
            return Err(ExactSelectorMerkleProofError::SchemaVersion);
        }
        if self.digest_algorithm != EXACT_SELECTOR_MERKLE_DIGEST_ALGORITHM {
            return Err(ExactSelectorMerkleProofError::DigestAlgorithm);
        }
        if self.language_id.trim().is_empty() {
            return Err(ExactSelectorMerkleProofError::LanguageId);
        }
        self.canonical_item_selector
            .validate()
            .map_err(|_| ExactSelectorMerkleProofError::CanonicalItemSelector)?;
        if self.canonical_item_selector.language_id != self.language_id
            || self.canonical_item_selector.structural_selector != self.structural_selector
        {
            return Err(ExactSelectorMerkleProofError::CanonicalItemSelector);
        }
        validate_owner_path(&self.owner_path)?;
        if !crate::workspace_merkle_v1::verify_owner_inclusion_v1(
            &self.owner_path,
            &self.source_blob_digest,
            &self.owner_subtree_digest,
            &self.owner_inclusion_proof,
            &self.workspace_root_digest,
        ) {
            return Err(ExactSelectorMerkleProofError::OwnerInclusion);
        }
        if self.structural_selector.trim().is_empty() {
            return Err(ExactSelectorMerkleProofError::StructuralSelector);
        }
        Ok(())
    }
}

pub fn derive_parser_fact_digest_v1(
    language_id: &str,
    parser_identity_digest: &ContentDigestV1,
    query_pack_digest: &ContentDigestV1,
    source_blob_digest: &ContentDigestV1,
    normalized_parser_facts: &[u8],
) -> ContentDigestV1 {
    canonical_digest_v1(
        b"asp.parser-fact.v1",
        &[
            language_id.as_bytes(),
            parser_identity_digest.as_str().as_bytes(),
            query_pack_digest.as_str().as_bytes(),
            source_blob_digest.as_str().as_bytes(),
            normalized_parser_facts,
        ],
    )
}

pub fn derive_projection_digest_v1(
    canonical_item_selector: &crate::canonical_item_identity::CanonicalItemSelectorV1,
    structural_selector: &str,
    projection_mode: ExactProjectionModeV1,
    parser_fact_digest: &ContentDigestV1,
    projection_payload: &[u8],
) -> ContentDigestV1 {
    let canonical_item_selector =
        serde_json::to_vec(canonical_item_selector).expect("canonical item selector v1 serializes");
    canonical_digest_v1(
        b"asp.exact-projection.v1",
        &[
            &canonical_item_selector,
            structural_selector.as_bytes(),
            projection_mode.as_str().as_bytes(),
            parser_fact_digest.as_str().as_bytes(),
            projection_payload,
        ],
    )
}

pub fn verify_projection_digest_v1(
    proof: &ExactSelectorMerkleProofV1,
    projection_payload: &[u8],
) -> Result<bool, ExactSelectorMerkleProofError> {
    proof.validate_shape()?;
    Ok(derive_projection_digest_v1(
        &proof.canonical_item_selector,
        &proof.structural_selector,
        proof.projection_mode,
        &proof.parser_fact_digest,
        projection_payload,
    ) == proof.projection_digest)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentDigestV1(String);

pub fn blake3_content_digest_v1(bytes: &[u8]) -> ContentDigestV1 {
    ContentDigestV1(blake3::hash(bytes).to_hex().to_string())
}

pub fn parse_content_digest_v1(value: &str) -> Result<ContentDigestV1, String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err("content digest must be 64 lowercase hexadecimal characters".to_string());
    }
    Ok(ContentDigestV1(value.to_string()))
}

pub fn canonical_content_digest_v1(domain: &[u8], parts: &[&[u8]]) -> ContentDigestV1 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(domain.len() as u64).to_be_bytes());
    hasher.update(domain);
    hasher.update(&(parts.len() as u64).to_be_bytes());
    for part in parts {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    ContentDigestV1(hasher.finalize().to_hex().to_string())
}

impl ContentDigestV1 {
    pub fn parse(value: impl Into<String>) -> Result<Self, ExactSelectorMerkleProofError> {
        let value = value.into();
        if value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            Ok(Self(value))
        } else {
            Err(ExactSelectorMerkleProofError::ContentDigest)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MerkleInclusionSideV1 {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MerkleInclusionStepV1 {
    pub side: MerkleInclusionSideV1,
    pub digest: ContentDigestV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExactProjectionModeV1 {
    Code,
    Skeleton,
    Names,
    Verbatim,
}

impl ExactProjectionModeV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Skeleton => "skeleton",
            Self::Names => "names",
            Self::Verbatim => "verbatim",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactSelectorMerkleProofError {
    SchemaId,
    SchemaVersion,
    DigestAlgorithm,
    LanguageId,
    OwnerPath,
    OwnerInclusion,
    ContentDigest,
    CanonicalItemSelector,
    StructuralSelector,
}

impl fmt::Display for ExactSelectorMerkleProofError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::SchemaId => "invalid exact-selector Merkle proof schema id",
            Self::SchemaVersion => "invalid exact-selector Merkle proof schema version",
            Self::DigestAlgorithm => "invalid exact-selector Merkle digest algorithm",
            Self::LanguageId => "languageId must be non-empty",
            Self::OwnerPath => "ownerPath must be a normalized relative path",
            Self::OwnerInclusion => "owner inclusion proof does not match workspace root",
            Self::ContentDigest => "content digest must be 64 lowercase hexadecimal characters",
            Self::CanonicalItemSelector => {
                "canonicalItemSelector must be valid and match languageId, ownerPath, and structuralSelector"
            }
            Self::StructuralSelector => "structuralSelector must be non-empty",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ExactSelectorMerkleProofError {}

fn validate_owner_path(owner_path: &str) -> Result<(), ExactSelectorMerkleProofError> {
    let path = std::path::Path::new(owner_path);
    if owner_path.trim().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(ExactSelectorMerkleProofError::OwnerPath);
    }
    Ok(())
}

pub(crate) fn canonical_digest_v1(domain: &[u8], parts: &[&[u8]]) -> ContentDigestV1 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(domain.len() as u64).to_be_bytes());
    hasher.update(domain);
    hasher.update(&(parts.len() as u64).to_be_bytes());
    for part in parts {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    ContentDigestV1(hasher.finalize().to_hex().to_string())
}
