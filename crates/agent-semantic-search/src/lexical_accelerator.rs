//! Generation-bound cold-rg and Tantivy accelerator admission.
//!
//! This module owns only identity/equivalence validation. Process execution is
//! Runtime-owned; file discovery, repository admission, and native syntax are
//! already complete before either route is admitted.

use serde::Deserialize;
use serde::Serialize;

use crate::LexicalGenerationPlan;
use crate::SearchGenerationIdentity;
use crate::canonical_blake3_digest;

pub const COLD_RG_QUERY_RECEIPT_SCHEMA_ID: &str = "agent.semantic-protocols.cold-rg-query-receipt";
pub const LEXICAL_ACCELERATOR_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.lexical-accelerator-receipt";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexicalRecallRoute {
    ColdRg,
    Tantivy,
}

/// Select the lexical executor from immutable published authority only.
///
/// A complete content generation is immediately searchable through its
/// resident cold corpus. Tantivy becomes eligible only when its independently
/// published receipt is bound to the exact content-generation digest. This
/// function performs no file discovery, process launch, or index build.
pub fn plan_lexical_recall_route(
    content: &crate::ContentSearchGenerationReceipt,
    accelerator: Option<&LexicalAcceleratorReceipt>,
) -> Result<LexicalRecallRoute, String> {
    content.validate()?;
    let Some(accelerator) = accelerator else {
        return Ok(LexicalRecallRoute::ColdRg);
    };
    accelerator.validate()?;
    if &accelerator.identity != content.identity()
        || accelerator.content_generation_digest != content.content_generation_digest
    {
        return Err("lexical accelerator content-generation identity drift".to_owned());
    }
    Ok(LexicalRecallRoute::Tantivy)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColdRgQueryReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub identity: SearchGenerationIdentity,
    pub inventory_digest: String,
    pub normalized_query_digest: String,
    pub candidate_set_digest: String,
    pub fd_process_count: u32,
    pub rg_process_count: u32,
    pub tantivy_build_count: u32,
    pub complete: bool,
}

impl ColdRgQueryReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != COLD_RG_QUERY_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
            || !self.complete
            || self.fd_process_count != 0
            || self.rg_process_count != 0
            || self.tantivy_build_count != 0
        {
            return Err("cold rg query receipt contract mismatch".to_owned());
        }
        self.identity.validate()?;
        validate_digest("inventoryDigest", &self.inventory_digest)?;
        validate_digest("normalizedQueryDigest", &self.normalized_query_digest)?;
        validate_digest("candidateSetDigest", &self.candidate_set_digest)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LexicalRouteEquivalenceCase {
    pub normalized_query_digest: String,
    pub cold_rg_candidate_set_digest: String,
    pub tantivy_candidate_set_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LexicalAcceleratorReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub identity: SearchGenerationIdentity,
    pub content_generation_digest: String,
    pub lexical_plan_digest: String,
    pub tantivy_artifact_digest: String,
    pub equivalence_cases: Vec<LexicalRouteEquivalenceCase>,
    pub equivalence_digest: String,
    pub complete: bool,
}

impl LexicalAcceleratorReceipt {
    pub fn build(
        content: &crate::ContentSearchGenerationReceipt,
        plan: &LexicalGenerationPlan,
        tantivy_artifact_digest: String,
        mut equivalence_cases: Vec<LexicalRouteEquivalenceCase>,
    ) -> Result<Self, String> {
        content.validate()?;
        plan.validate()?;
        validate_digest("tantivyArtifactDigest", &tantivy_artifact_digest)?;
        equivalence_cases.sort_by(|left, right| {
            left.normalized_query_digest
                .cmp(&right.normalized_query_digest)
        });
        validate_equivalence_cases(&equivalence_cases)?;
        let equivalence_digest = digest_json("lexical route equivalence", &equivalence_cases)?;
        let receipt = Self {
            schema_id: LEXICAL_ACCELERATOR_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            identity: content.identity().clone(),
            content_generation_digest: content.content_generation_digest.clone(),
            lexical_plan_digest: plan.plan_digest.clone(),
            tantivy_artifact_digest,
            equivalence_cases,
            equivalence_digest,
            complete: true,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != LEXICAL_ACCELERATOR_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
            || !self.complete
        {
            return Err("lexical accelerator receipt contract mismatch".to_owned());
        }
        self.identity.validate()?;
        validate_digest("contentGenerationDigest", &self.content_generation_digest)?;
        validate_digest("lexicalPlanDigest", &self.lexical_plan_digest)?;
        validate_digest("tantivyArtifactDigest", &self.tantivy_artifact_digest)?;
        validate_equivalence_cases(&self.equivalence_cases)?;
        let expected = digest_json("lexical route equivalence", &self.equivalence_cases)?;
        if self.equivalence_digest != expected {
            return Err("lexical accelerator equivalence digest drift".to_owned());
        }
        Ok(())
    }
}

fn validate_equivalence_cases(cases: &[LexicalRouteEquivalenceCase]) -> Result<(), String> {
    if cases.is_empty()
        || cases
            .windows(2)
            .any(|window| window[0].normalized_query_digest >= window[1].normalized_query_digest)
    {
        return Err("lexical accelerator equivalence corpus is empty or non-canonical".to_owned());
    }
    for case in cases {
        validate_digest("normalizedQueryDigest", &case.normalized_query_digest)?;
        validate_digest(
            "coldRgCandidateSetDigest",
            &case.cold_rg_candidate_set_digest,
        )?;
        validate_digest(
            "tantivyCandidateSetDigest",
            &case.tantivy_candidate_set_digest,
        )?;
        if case.cold_rg_candidate_set_digest != case.tantivy_candidate_set_digest {
            return Err("cold rg and Tantivy candidate sets are not equivalent".to_owned());
        }
    }
    Ok(())
}

fn validate_digest(field: &str, digest: &str) -> Result<(), String> {
    match canonical_blake3_digest(digest) {
        Ok(canonical) if canonical == digest => Ok(()),
        _ => Err(format!("{field} is not a canonical BLAKE3 digest")),
    }
}

fn digest_json(label: &str, value: &impl Serialize) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|bytes| format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
        .map_err(|error| format!("encode {label}: {error}"))
}
