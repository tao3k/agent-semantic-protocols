// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Runtime-owned evidence identity shared by compact derived projections.

use serde::Deserialize;
use serde::Serialize;

use crate::LanguageIdV1;
use crate::ProjectionEvidenceContextRefV1;
use crate::ProviderIdV1;
use crate::exact_selector_merkle::ExactSelectorMerkleProofV1;
use crate::exact_structural_selector::ExactStructuralSelectorV1;

/// Schema identifier for content-bound projection evidence.
pub const PROJECTION_EVIDENCE_CONTEXT_SCHEMA_ID: &str =
    "agent.semantic-protocols.projection-evidence-context";
/// Schema version for projection evidence contexts.
pub const PROJECTION_EVIDENCE_CONTEXT_SCHEMA_VERSION: &str = "1";
/// Schema identifier for an exact structural-selector reference.
pub const EXACT_STRUCTURAL_SELECTOR_REFERENCE_SCHEMA_ID: &str =
    "asp.exact-structural-selector-reference.v1";

/// Content-addressed Runtime identity interned once in an immutable generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectionEvidenceContext {
    pub schema_id: String,
    pub schema_version: String,
    pub evidence_context_ref: ProjectionEvidenceContextRefV1,
    pub language_id: LanguageIdV1,
    pub provider_id: ProviderIdV1,
    pub generation_identity_digest: String,
    pub parser_identity_digest: String,
    pub query_pack_digest: String,
}

/// Compact selector identity resolved against a Runtime evidence context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExactStructuralSelectorReferenceV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub selector: String,
    pub evidence_context_ref: String,
}

impl ProjectionEvidenceContext {
    pub fn from_exact_selector_proof(
        provider_id: impl Into<ProviderIdV1>,
        proof: &ExactSelectorMerkleProofV1,
    ) -> Self {
        let provider_id = provider_id.into();
        let language_id = proof.language_id();
        let generation_identity_digest = proof.workspace_root_digest().as_str();
        let parser_identity_digest = proof.parser_identity_digest().as_str();
        let query_pack_digest = proof.query_pack_digest().as_str();
        let evidence_context_ref = evidence_context_ref(
            language_id,
            provider_id.as_str(),
            generation_identity_digest,
            parser_identity_digest,
            query_pack_digest,
        );
        Self {
            schema_id: PROJECTION_EVIDENCE_CONTEXT_SCHEMA_ID.to_owned(),
            schema_version: PROJECTION_EVIDENCE_CONTEXT_SCHEMA_VERSION.to_owned(),
            evidence_context_ref: evidence_context_ref.into(),
            language_id: language_id.into(),
            provider_id,
            generation_identity_digest: generation_identity_digest.to_owned(),
            parser_identity_digest: parser_identity_digest.to_owned(),
            query_pack_digest: query_pack_digest.to_owned(),
        }
    }

    pub fn from_inline_selector(
        provider_id: impl Into<ProviderIdV1>,
        selector: &ExactStructuralSelectorV1,
    ) -> Result<Self, ProjectionEvidenceContextValidationError> {
        let provider_id = provider_id.into();
        selector
            .validate()
            .map_err(|_| ProjectionEvidenceContextValidationError::InlineSelector)?;
        let evidence_context_ref = evidence_context_ref(
            &selector.language_id,
            provider_id.as_str(),
            &selector.generation_identity_digest,
            &selector.parser_identity_digest,
            &selector.query_pack_digest,
        );
        Ok(Self {
            schema_id: PROJECTION_EVIDENCE_CONTEXT_SCHEMA_ID.to_owned(),
            schema_version: PROJECTION_EVIDENCE_CONTEXT_SCHEMA_VERSION.to_owned(),
            evidence_context_ref: evidence_context_ref.into(),
            language_id: selector.language_id.as_str().into(),
            provider_id,
            generation_identity_digest: selector.generation_identity_digest.clone(),
            parser_identity_digest: selector.parser_identity_digest.clone(),
            query_pack_digest: selector.query_pack_digest.clone(),
        })
    }

    pub fn validate(&self) -> Result<(), ProjectionEvidenceContextValidationError> {
        if self.schema_id != PROJECTION_EVIDENCE_CONTEXT_SCHEMA_ID {
            return Err(ProjectionEvidenceContextValidationError::SchemaId);
        }
        if self.schema_version != PROJECTION_EVIDENCE_CONTEXT_SCHEMA_VERSION {
            return Err(ProjectionEvidenceContextValidationError::SchemaVersion);
        }
        if self.language_id.as_str().is_empty() || self.provider_id.as_str().is_empty() {
            return Err(ProjectionEvidenceContextValidationError::EmptyIdentity);
        }
        for digest in [
            &self.generation_identity_digest,
            &self.parser_identity_digest,
            &self.query_pack_digest,
        ] {
            if !is_plain_blake3_digest(digest) {
                return Err(ProjectionEvidenceContextValidationError::Digest);
            }
        }
        let expected = evidence_context_ref(
            self.language_id.as_str(),
            self.provider_id.as_str(),
            &self.generation_identity_digest,
            &self.parser_identity_digest,
            &self.query_pack_digest,
        );
        if self.evidence_context_ref.as_str() != expected {
            return Err(ProjectionEvidenceContextValidationError::ContextRef);
        }
        Ok(())
    }

    pub fn selector_reference(&self, selector: String) -> ExactStructuralSelectorReferenceV1 {
        ExactStructuralSelectorReferenceV1 {
            schema_id: EXACT_STRUCTURAL_SELECTOR_REFERENCE_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            selector,
            evidence_context_ref: self.evidence_context_ref.to_string(),
        }
    }
}

impl ExactStructuralSelectorReferenceV1 {
    pub fn validate(&self) -> Result<(), ProjectionEvidenceContextValidationError> {
        if self.schema_id != EXACT_STRUCTURAL_SELECTOR_REFERENCE_SCHEMA_ID {
            return Err(ProjectionEvidenceContextValidationError::ReferenceSchemaId);
        }
        if self.schema_version != "1" {
            return Err(ProjectionEvidenceContextValidationError::SchemaVersion);
        }
        if self.selector.is_empty()
            || !self.selector.contains("://")
            || !self.selector.contains('#')
        {
            return Err(ProjectionEvidenceContextValidationError::Selector);
        }
        if !is_context_ref(&self.evidence_context_ref) {
            return Err(ProjectionEvidenceContextValidationError::ContextRef);
        }
        Ok(())
    }
}

fn evidence_context_ref(
    language_id: &str,
    provider_id: &str,
    generation_identity_digest: &str,
    parser_identity_digest: &str,
    query_pack_digest: &str,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"asp.projection-evidence-context.v1\0");
    for component in [
        language_id,
        provider_id,
        generation_identity_digest,
        parser_identity_digest,
        query_pack_digest,
    ] {
        hasher.update(&(component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

fn is_plain_blake3_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_context_ref(value: &str) -> bool {
    value
        .strip_prefix("blake3-256:")
        .is_some_and(is_plain_blake3_digest)
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Deterministic validation error for projection evidence.
pub enum ProjectionEvidenceContextValidationError {
    SchemaId,
    ReferenceSchemaId,
    SchemaVersion,
    EmptyIdentity,
    Digest,
    ContextRef,
    Selector,
    InlineSelector,
}

impl std::fmt::Display for ProjectionEvidenceContextValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProjectionEvidenceContextValidationError {}
