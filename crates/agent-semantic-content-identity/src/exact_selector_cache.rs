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
        verify_owner_inclusion: impl FnOnce(&ExactSelectorMerkleProofV1) -> bool,
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

        if !verify_owner_inclusion(&self.proof) {
            return Err(ExactSelectorMerkleMissV1::OwnerInclusionMismatch);
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
