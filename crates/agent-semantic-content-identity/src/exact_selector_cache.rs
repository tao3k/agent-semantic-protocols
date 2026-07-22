use crate::exact_selector_merkle::{
    ContentDigestV1, ExactProjectionModeV1, ExactSelectorMerkleProofV1, verify_projection_digest_v1,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactSelectorMerkleLookupKeyV1<'a> {
    pub language_id: &'a str,
    pub workspace_root_digest: &'a ContentDigestV1,
    pub owner_path: &'a str,
    pub owner_subtree_digest: &'a ContentDigestV1,
    pub source_blob_digest: &'a ContentDigestV1,
    pub parser_identity_digest: &'a ContentDigestV1,
    pub query_pack_digest: &'a ContentDigestV1,
    pub structural_selector: &'a str,
    pub projection_mode: ExactProjectionModeV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExactSelectorProjectionRecordV1 {
    pub proof: ExactSelectorMerkleProofV1,
    pub projection_payload: Vec<u8>,
}

impl ExactSelectorProjectionRecordV1 {
    pub fn validate_warm_hit<'a>(
        &'a self,
        key: &ExactSelectorMerkleLookupKeyV1<'_>,
    ) -> Result<ExactSelectorWarmHitV1<'a>, ExactSelectorMerkleMissV1> {
        self.proof
            .validate_shape()
            .map_err(|_| ExactSelectorMerkleMissV1::InvalidProofShape)?;

        if self.proof.language_id != key.language_id
            || &self.proof.workspace_root_digest != key.workspace_root_digest
            || self.proof.owner_path != key.owner_path
            || &self.proof.owner_subtree_digest != key.owner_subtree_digest
            || &self.proof.source_blob_digest != key.source_blob_digest
            || &self.proof.parser_identity_digest != key.parser_identity_digest
            || &self.proof.query_pack_digest != key.query_pack_digest
            || self.proof.structural_selector != key.structural_selector
            || self.proof.projection_mode != key.projection_mode
        {
            return Err(ExactSelectorMerkleMissV1::IdentityMismatch);
        }

        match verify_projection_digest_v1(&self.proof, &self.projection_payload) {
            Ok(true) => Ok(ExactSelectorWarmHitV1 {
                proof: &self.proof,
                projection_payload: &self.projection_payload,
                side_effects: ExactSelectorWarmSideEffectsV1::ZERO,
            }),
            Ok(false) => Err(ExactSelectorMerkleMissV1::ProjectionDigestMismatch),
            Err(_) => Err(ExactSelectorMerkleMissV1::InvalidProofShape),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactSelectorWarmHitV1<'a> {
    pub proof: &'a ExactSelectorMerkleProofV1,
    pub projection_payload: &'a [u8],
    pub side_effects: ExactSelectorWarmSideEffectsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExactSelectorWarmSideEffectsV1 {
    pub parser_process_count: u64,
    pub content_store_write_count: u64,
    pub turso_write_count: u64,
    pub manifest_write_count: u64,
}

impl ExactSelectorWarmSideEffectsV1 {
    pub const ZERO: Self = Self {
        parser_process_count: 0,
        content_store_write_count: 0,
        turso_write_count: 0,
        manifest_write_count: 0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExactSelectorMerkleMissV1 {
    NotFound,
    InvalidProofShape,
    IdentityMismatch,
    OwnerInclusionMismatch,
    ProjectionDigestMismatch,
}

/// An owned exact-selector projection whose proof was verified at the
/// persistence or hydration boundary.
///
/// The record is private so callers cannot construct a warm-cache entry without
/// first passing the complete Merkle and projection validation performed by
/// [`ExactSelectorProjectionRecordV1::validate_warm_hit`]. Subsequent lookups
/// only bind the immutable record to the current typed identity key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedExactSelectorProjectionV1 {
    record: ExactSelectorProjectionRecordV1,
}

impl ValidatedExactSelectorProjectionV1 {
    pub fn hydrate(
        record: ExactSelectorProjectionRecordV1,
        key: &ExactSelectorMerkleLookupKeyV1<'_>,
    ) -> Result<Self, ExactSelectorMerkleMissV1> {
        record.validate_warm_hit(key)?;
        Ok(Self { record })
    }

    pub fn validate_warm_hit(
        &self,
        key: &ExactSelectorMerkleLookupKeyV1<'_>,
    ) -> Result<ExactSelectorWarmHitV1<'_>, ExactSelectorMerkleMissV1> {
        if !lookup_key_matches_proof(key, &self.record.proof) {
            return Err(ExactSelectorMerkleMissV1::IdentityMismatch);
        }
        Ok(ExactSelectorWarmHitV1 {
            proof: &self.record.proof,
            projection_payload: &self.record.projection_payload,
            side_effects: ExactSelectorWarmSideEffectsV1::ZERO,
        })
    }

    pub fn record(&self) -> &ExactSelectorProjectionRecordV1 {
        &self.record
    }
}

fn lookup_key_matches_proof(
    key: &ExactSelectorMerkleLookupKeyV1<'_>,
    proof: &ExactSelectorMerkleProofV1,
) -> bool {
    key.language_id == proof.language_id
        && key.workspace_root_digest == &proof.workspace_root_digest
        && key.owner_path == proof.owner_path
        && key.owner_subtree_digest == &proof.owner_subtree_digest
        && key.source_blob_digest == &proof.source_blob_digest
        && key.parser_identity_digest == &proof.parser_identity_digest
        && key.query_pack_digest == &proof.query_pack_digest
        && key.structural_selector == proof.structural_selector
        && key.projection_mode == proof.projection_mode
}
