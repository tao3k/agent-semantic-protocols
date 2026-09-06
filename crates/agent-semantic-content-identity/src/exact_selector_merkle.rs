// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Exact-selector Merkle proofs and domain-separated content digests.

use serde::Deserialize;
use serde::Serialize;
use std::fmt;

/// Schema identifier for an exact-selector Merkle proof.
pub const EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_ID: &str =
    "agent.semantic-protocols.exact-selector-merkle-proof";
/// Schema version for an exact-selector Merkle proof.
pub const EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_VERSION: &str = "1";
/// Digest algorithm used by exact-selector Merkle proofs.
pub const EXACT_SELECTOR_MERKLE_DIGEST_ALGORITHM: &str = "blake3-256";

/// Provider parser language identity retained in exact-selector proofs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ParserLanguageIdV1(String);

impl ParserLanguageIdV1 {
    /// Creates a parser language identity.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    /// Returns the parser language identity as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ParserLanguageIdV1 {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for ParserLanguageIdV1 {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Content-proven exact-selector projection receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExactSelectorMerkleProofV1 {
    pub(crate) schema_id: String,
    pub(crate) schema_version: String,
    pub(crate) digest_algorithm: String,
    pub(crate) language_id: ParserLanguageIdV1,
    pub(crate) workspace_root_digest: ContentDigestV1,
    pub(crate) owner_path: String,
    pub(crate) owner_subtree_digest: ContentDigestV1,
    pub(crate) owner_inclusion_proof: Vec<MerkleInclusionStepV1>,
    pub(crate) source_blob_digest: ContentDigestV1,
    pub(crate) parser_identity_digest: ContentDigestV1,
    pub(crate) query_pack_digest: ContentDigestV1,
    pub(crate) parser_fact_digest: ContentDigestV1,
    pub(crate) canonical_item_selector: crate::canonical_item_identity::CanonicalItemSelector,
    pub(crate) structural_selector: String,
    pub(crate) projection_mode: ExactProjectionModeV1,
    pub(crate) projection_digest: ContentDigestV1,
}

pub(crate) struct ExactSelectorMerkleProofInputV1 {
    pub(crate) language_id: ParserLanguageIdV1,
    pub(crate) workspace_root_digest: ContentDigestV1,
    pub(crate) owner_path: String,
    pub(crate) owner_subtree_digest: ContentDigestV1,
    pub(crate) owner_inclusion_proof: Vec<MerkleInclusionStepV1>,
    pub(crate) source_blob_digest: ContentDigestV1,
    pub(crate) parser_identity_digest: ContentDigestV1,
    pub(crate) query_pack_digest: ContentDigestV1,
    pub(crate) parser_fact_digest: ContentDigestV1,
    pub(crate) canonical_item_selector: crate::canonical_item_identity::CanonicalItemSelector,
    pub(crate) structural_selector: String,
    pub(crate) projection_mode: ExactProjectionModeV1,
    pub(crate) projection_digest: ContentDigestV1,
}

impl ExactSelectorMerkleProofV1 {
    pub(crate) fn from_input(input: ExactSelectorMerkleProofInputV1) -> Self {
        Self {
            schema_id: EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_ID.to_owned(),
            schema_version: EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_VERSION.to_owned(),
            digest_algorithm: EXACT_SELECTOR_MERKLE_DIGEST_ALGORITHM.to_owned(),
            language_id: input.language_id,
            workspace_root_digest: input.workspace_root_digest,
            owner_path: input.owner_path,
            owner_subtree_digest: input.owner_subtree_digest,
            owner_inclusion_proof: input.owner_inclusion_proof,
            source_blob_digest: input.source_blob_digest,
            parser_identity_digest: input.parser_identity_digest,
            query_pack_digest: input.query_pack_digest,
            parser_fact_digest: input.parser_fact_digest,
            canonical_item_selector: input.canonical_item_selector,
            structural_selector: input.structural_selector,
            projection_mode: input.projection_mode,
            projection_digest: input.projection_digest,
        }
    }

    /// Validates schema identity, owner inclusion, and selector binding.
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
        if self.language_id.as_str().trim().is_empty() {
            return Err(ExactSelectorMerkleProofError::LanguageId);
        }
        self.canonical_item_selector
            .validate()
            .map_err(|_| ExactSelectorMerkleProofError::CanonicalItemSelector)?;
        if self.canonical_item_selector.language_id.as_str() != self.language_id.as_str()
            || self.canonical_item_selector.structural_selector != self.structural_selector
        {
            return Err(ExactSelectorMerkleProofError::CanonicalItemSelector);
        }
        validate_owner_path(&self.owner_path)?;
        if !crate::workspace_merkle_v1::verify_owner_inclusion_v1(
            crate::workspace_merkle_v1::WorkspaceOwnerInclusionV1 {
                owner_path: &self.owner_path,
                source_blob_digest: &self.source_blob_digest,
                expected_owner_subtree_digest: &self.owner_subtree_digest,
                inclusion_proof: &self.owner_inclusion_proof,
                expected_workspace_root_digest: &self.workspace_root_digest,
            },
        ) {
            return Err(ExactSelectorMerkleProofError::OwnerInclusion);
        }
        if self.structural_selector.trim().is_empty() {
            return Err(ExactSelectorMerkleProofError::StructuralSelector);
        }
        Ok(())
    }

    /// Returns the provider language identity.
    pub fn language_id(&self) -> &str {
        self.language_id.as_str()
    }

    /// Returns the typed parser language identity.
    pub fn parser_language_id(&self) -> &ParserLanguageIdV1 {
        &self.language_id
    }

    /// Returns the admitted workspace root digest.
    pub fn workspace_root_digest(&self) -> &ContentDigestV1 {
        &self.workspace_root_digest
    }

    /// Returns the normalized source owner path.
    pub fn owner_path(&self) -> &str {
        &self.owner_path
    }

    /// Returns the owner subtree digest.
    pub fn owner_subtree_digest(&self) -> &ContentDigestV1 {
        &self.owner_subtree_digest
    }

    /// Returns the owner inclusion proof.
    pub fn owner_inclusion_proof(&self) -> &[MerkleInclusionStepV1] {
        &self.owner_inclusion_proof
    }

    /// Returns the exact source blob digest.
    pub fn source_blob_digest(&self) -> &ContentDigestV1 {
        &self.source_blob_digest
    }

    /// Returns the parser implementation identity digest.
    pub fn parser_identity_digest(&self) -> &ContentDigestV1 {
        &self.parser_identity_digest
    }

    /// Returns the provider query-pack digest.
    pub fn query_pack_digest(&self) -> &ContentDigestV1 {
        &self.query_pack_digest
    }

    /// Returns the normalized parser-fact digest.
    pub fn parser_fact_digest(&self) -> &ContentDigestV1 {
        &self.parser_fact_digest
    }

    /// Returns the canonical item selector bound by this proof.
    pub fn canonical_item_selector(
        &self,
    ) -> &crate::canonical_item_identity::CanonicalItemSelector {
        &self.canonical_item_selector
    }

    /// Returns the structural selector bound by this proof.
    pub fn structural_selector(&self) -> &str {
        &self.structural_selector
    }

    /// Returns the exact projection mode.
    pub fn projection_mode(&self) -> &ExactProjectionModeV1 {
        &self.projection_mode
    }

    /// Returns the exact projection payload digest.
    pub fn projection_digest(&self) -> &ContentDigestV1 {
        &self.projection_digest
    }
}

/// Named inputs for deriving one normalized parser-fact digest.
pub struct ParserFactDigestInputV1<'a> {
    pub language_id: &'a ParserLanguageIdV1,
    pub parser_identity_digest: &'a ContentDigestV1,
    pub query_pack_digest: &'a ContentDigestV1,
    pub source_blob_digest: &'a ContentDigestV1,
    pub normalized_parser_facts: &'a [u8],
}

/// Derives the domain-separated digest of normalized provider parser facts.
pub fn derive_parser_fact_digest_v1(input: ParserFactDigestInputV1<'_>) -> ContentDigestV1 {
    canonical_digest_v1(
        b"asp.parser-fact.v1",
        &[
            input.language_id.as_str().as_bytes(),
            input.parser_identity_digest.as_str().as_bytes(),
            input.query_pack_digest.as_str().as_bytes(),
            input.source_blob_digest.as_str().as_bytes(),
            input.normalized_parser_facts,
        ],
    )
}

/// Named inputs for deriving one exact projection digest.
pub struct ProjectionDigestInputV1<'a> {
    pub canonical_item_selector: &'a crate::canonical_item_identity::CanonicalItemSelector,
    pub structural_selector: &'a str,
    pub projection_mode: ExactProjectionModeV1,
    pub parser_fact_digest: &'a ContentDigestV1,
    pub projection_payload: &'a [u8],
}

/// Derives the domain-separated digest of one exact projection payload.
pub fn derive_projection_digest_v1(input: ProjectionDigestInputV1<'_>) -> ContentDigestV1 {
    let canonical_item_selector = serde_json::to_vec(input.canonical_item_selector)
        .expect("canonical item selector v1 serializes");
    canonical_digest_v1(
        b"asp.exact-projection.v1",
        &[
            &canonical_item_selector,
            input.structural_selector.as_bytes(),
            input.projection_mode.as_str().as_bytes(),
            input.parser_fact_digest.as_str().as_bytes(),
            input.projection_payload,
        ],
    )
}

/// Verifies an exact projection payload against its Merkle proof.
pub fn verify_projection_digest_v1(
    proof: &ExactSelectorMerkleProofV1,
    projection_payload: &[u8],
) -> Result<bool, ExactSelectorMerkleProofError> {
    proof.validate_shape()?;
    Ok(derive_projection_digest_v1(ProjectionDigestInputV1 {
        canonical_item_selector: &proof.canonical_item_selector,
        structural_selector: &proof.structural_selector,
        projection_mode: proof.projection_mode,
        parser_fact_digest: &proof.parser_fact_digest,
        projection_payload,
    }) == proof.projection_digest)
}

/// Canonical lowercase BLAKE3 content digest.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentDigestV1(String);

/// Hashes one byte slice into a canonical content digest.
pub fn blake3_content_digest_v1(bytes: &[u8]) -> ContentDigestV1 {
    ContentDigestV1(blake3::hash(bytes).to_hex().to_string())
}

/// Parses a canonical lowercase hexadecimal content digest.
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

/// Hashes length-delimited parts under an explicit identity domain.
pub fn canonical_content_digest(domain: &[u8], parts: &[&[u8]]) -> ContentDigestV1 {
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
    /// Parses a canonical content digest into the proof error domain.
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

    /// Returns the lowercase hexadecimal digest.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Side of a sibling digest in a Merkle inclusion step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MerkleInclusionSideV1 {
    Left,
    Right,
}

/// One sibling step in an owner inclusion proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MerkleInclusionStepV1 {
    pub side: MerkleInclusionSideV1,
    pub digest: ContentDigestV1,
}

/// Projection form whose bytes are bound by an exact-selector proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExactProjectionModeV1 {
    Code,
    Skeleton,
    Names,
    Verbatim,
}

impl ExactProjectionModeV1 {
    /// Returns the stable wire label for this projection mode.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Skeleton => "skeleton",
            Self::Names => "names",
            Self::Verbatim => "verbatim",
        }
    }
}

/// Typed validation failures for exact-selector Merkle proofs.
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
