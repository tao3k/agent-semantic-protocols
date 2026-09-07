// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Exact identity and completion contract for resident content generations.

use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

pub const CONTENT_SEARCH_GENERATION_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.content-search-generation-receipt";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchGenerationConstructionStage {
    SourceByteAcquisition,
    NativeSyntax,
    ResidentGraph,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchGenerationIdentity {
    pub project_id: String,
    pub workspace_id: String,
    pub source_root_digest: String,
    pub provider_digest: String,
    pub schema_digest: String,
    pub generation_candidate_digest: String,
}

impl SearchGenerationIdentity {
    pub fn validate(&self) -> Result<(), String> {
        if self.project_id.trim().is_empty() || self.workspace_id.trim().is_empty() {
            return Err("search generation project/workspace identity is incomplete".to_owned());
        }
        validate_digest("sourceRootDigest", &self.source_root_digest)?;
        validate_digest("providerDigest", &self.provider_digest)?;
        validate_digest("schemaDigest", &self.schema_digest)?;
        validate_digest(
            "generationCandidateDigest",
            &self.generation_candidate_digest,
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchGenerationStageReceipt {
    pub stage: SearchGenerationConstructionStage,
    pub identity: SearchGenerationIdentity,
    pub artifact_digest: String,
    pub worker_id: String,
    pub complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceByteOwner<'a> {
    pub owner_path: &'a str,
    pub content_digest: &'a str,
    pub byte_len: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeSyntaxSelector {
    pub selector: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub query_keys: Vec<String>,
    pub derived_projection_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeSyntaxProjection {
    pub owner_path: String,
    pub content_digest: String,
    pub selectors: Vec<NativeSyntaxSelector>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeSyntaxRelation {
    pub owner_path: String,
    pub relation_digest: String,
}

/// Provider-owned evidence that one admitted owner could not be projected.
///
/// This is generation evidence, not a synthetic selector.  It keeps owner
/// accounting total while allowing the independent rg, lexical, and graph
/// lanes to retain their valid results.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeSyntaxDiagnostic {
    pub owner_path: String,
    pub content_digest: String,
    pub reason_kind: String,
    pub message: String,
}

/// Commit complete byte membership for the resident source lane.
///
/// This is constructed once from the same immutable owner bytes as the
/// lexical index. Ready queries use the resident bytes directly and never
/// launch an `rg` process or rescan the workspace.
pub fn build_source_byte_acquisition_stage<'a>(
    identity: SearchGenerationIdentity,
    owners: impl IntoIterator<Item = SourceByteOwner<'a>>,
) -> Result<SearchGenerationStageReceipt, String> {
    let mut owners = owners.into_iter().collect::<Vec<_>>();
    owners.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    if owners
        .windows(2)
        .any(|window| window[0].owner_path == window[1].owner_path)
    {
        return Err("source-byte coverage contains duplicate owner paths".to_owned());
    }
    for owner in &owners {
        if owner.owner_path.trim().is_empty() {
            return Err("source-byte coverage contains an incomplete owner".to_owned());
        }
        validate_digest("contentDigest", &owner.content_digest)?;
    }
    let bytes = serde_json::to_vec(&owners)
        .map_err(|error| format!("encode source-byte coverage membership: {error}"))?;
    Ok(SearchGenerationStageReceipt {
        stage: SearchGenerationConstructionStage::SourceByteAcquisition,
        identity,
        artifact_digest: format!("blake3-256:{}", blake3::hash(&bytes).to_hex()),
        worker_id: "rust-source-byte-acquisition-v1".to_owned(),
        complete: true,
    })
}

/// Commit the complete provider-native syntax projection used by later stages.
///
/// The receipt binds owner identity, canonical selectors, byte ranges, parser
/// query keys, derived projections, and relations to the same immutable source
/// bytes as acquisition. It is deliberately not a public command surface: the
/// single Search playbook owns its execution.
pub fn build_native_syntax_stage(
    identity: SearchGenerationIdentity,
    projections: impl IntoIterator<Item = NativeSyntaxProjection>,
    relations: impl IntoIterator<Item = NativeSyntaxRelation>,
) -> Result<SearchGenerationStageReceipt, String> {
    build_native_syntax_stage_with_diagnostics(identity, projections, relations, [])
}

pub fn build_native_syntax_stage_with_diagnostics(
    identity: SearchGenerationIdentity,
    projections: impl IntoIterator<Item = NativeSyntaxProjection>,
    relations: impl IntoIterator<Item = NativeSyntaxRelation>,
    diagnostics: impl IntoIterator<Item = NativeSyntaxDiagnostic>,
) -> Result<SearchGenerationStageReceipt, String> {
    let (projections, owner_paths) = canonical_native_syntax_projections(projections)?;
    let relations = canonical_native_syntax_relations(relations, &owner_paths)?;
    let diagnostics = canonical_native_syntax_diagnostics(diagnostics, &owner_paths)?;
    let bytes = serde_json::to_vec(&(projections, relations, diagnostics))
        .map_err(|error| format!("encode native syntax playbook: {error}"))?;
    Ok(SearchGenerationStageReceipt {
        stage: SearchGenerationConstructionStage::NativeSyntax,
        identity,
        artifact_digest: format!("blake3-256:{}", blake3::hash(&bytes).to_hex()),
        worker_id: "provider-native-syntax-playbook-v1".to_owned(),
        complete: true,
    })
}

fn canonical_native_syntax_projections(
    projections: impl IntoIterator<Item = NativeSyntaxProjection>,
) -> Result<(Vec<NativeSyntaxProjection>, BTreeSet<String>), String> {
    let mut projections = projections.into_iter().collect::<Vec<_>>();
    projections.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    if projections
        .windows(2)
        .any(|window| window[0].owner_path == window[1].owner_path)
    {
        return Err("native syntax playbook contains duplicate owner paths".to_owned());
    }
    for projection in &mut projections {
        canonicalize_native_syntax_projection(projection)?;
    }
    let owner_paths = projections
        .iter()
        .map(|owner| owner.owner_path.clone())
        .collect::<BTreeSet<_>>();
    Ok((projections, owner_paths))
}

fn canonicalize_native_syntax_projection(
    projection: &mut NativeSyntaxProjection,
) -> Result<(), String> {
    if projection.owner_path.trim().is_empty() {
        return Err("native syntax playbook contains an incomplete owner".to_owned());
    }
    validate_digest("contentDigest", &projection.content_digest)?;
    projection
        .selectors
        .sort_by(|left, right| left.selector.cmp(&right.selector));
    if projection
        .selectors
        .windows(2)
        .any(|window| window[0].selector == window[1].selector)
    {
        return Err("native syntax playbook contains duplicate selectors".to_owned());
    }
    for selector in &mut projection.selectors {
        canonicalize_native_syntax_selector(selector)?;
    }
    Ok(())
}

fn canonicalize_native_syntax_selector(selector: &mut NativeSyntaxSelector) -> Result<(), String> {
    if selector.selector.trim().is_empty() || selector.byte_start >= selector.byte_end {
        return Err("native syntax playbook contains an invalid selector".to_owned());
    }
    validate_digest(
        "derivedProjectionDigest",
        &selector.derived_projection_digest,
    )?;
    selector.query_keys.sort_unstable();
    selector.query_keys.dedup();
    if selector.query_keys.is_empty() || selector.query_keys.iter().any(|key| key.trim().is_empty())
    {
        return Err("native syntax playbook selector has no query keys".to_owned());
    }
    Ok(())
}

fn canonical_native_syntax_relations(
    relations: impl IntoIterator<Item = NativeSyntaxRelation>,
    owner_paths: &BTreeSet<String>,
) -> Result<Vec<NativeSyntaxRelation>, String> {
    let mut relations = relations.into_iter().collect::<Vec<_>>();
    relations.sort_by(|left, right| {
        left.owner_path
            .cmp(&right.owner_path)
            .then_with(|| left.relation_digest.cmp(&right.relation_digest))
    });
    if relations.windows(2).any(|window| window[0] == window[1]) {
        return Err("native syntax playbook contains duplicate relations".to_owned());
    }
    for relation in &relations {
        if relation.owner_path.trim().is_empty() {
            return Err("native syntax playbook contains an incomplete relation owner".to_owned());
        }
        if !owner_paths.contains(&relation.owner_path) {
            return Err("native syntax playbook relation references an unknown owner".to_owned());
        }
        validate_digest("relationDigest", &relation.relation_digest)?;
    }
    Ok(relations)
}

fn canonical_native_syntax_diagnostics(
    diagnostics: impl IntoIterator<Item = NativeSyntaxDiagnostic>,
    owner_paths: &BTreeSet<String>,
) -> Result<Vec<NativeSyntaxDiagnostic>, String> {
    let mut diagnostics = diagnostics.into_iter().collect::<Vec<_>>();
    diagnostics.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    if diagnostics
        .windows(2)
        .any(|window| window[0].owner_path == window[1].owner_path)
    {
        return Err("native syntax playbook contains duplicate diagnostics".to_owned());
    }
    for diagnostic in &diagnostics {
        if diagnostic.owner_path.trim().is_empty()
            || owner_paths.contains(&diagnostic.owner_path)
            || diagnostic.reason_kind != "source-syntax-unavailable"
            || diagnostic.message.trim().is_empty()
        {
            return Err("native syntax playbook contains an invalid diagnostic".to_owned());
        }
        validate_digest("contentDigest", &diagnostic.content_digest)?;
    }
    Ok(diagnostics)
}

impl SearchGenerationStageReceipt {
    pub fn validate_for(&self, stage: SearchGenerationConstructionStage) -> Result<(), String> {
        if self.stage != stage || !self.complete || self.worker_id.trim().is_empty() {
            return Err(format!("search generation stage is incomplete: {stage:?}"));
        }
        self.identity.validate()?;
        validate_digest("artifactDigest", &self.artifact_digest)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentSearchGenerationReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub acquisition: SearchGenerationStageReceipt,
    pub content_generation_digest: String,
}

impl ContentSearchGenerationReceipt {
    /// Publishes the immutable byte-complete generation used by every Search lane.
    ///
    /// Provider-native syntax, Tantivy, and graph indexes are exact-generation
    /// derived attachments. Their latency or failure must not participate in
    /// the base content-generation digest or block cold byte recall.
    pub fn new(acquisition: SearchGenerationStageReceipt) -> Result<Self, String> {
        let content_generation_digest = content_generation_digest(&acquisition)?;
        let receipt = Self {
            schema_id: CONTENT_SEARCH_GENERATION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            acquisition,
            content_generation_digest,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != CONTENT_SEARCH_GENERATION_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err("content search generation receipt schema mismatch".to_owned());
        }
        self.acquisition
            .validate_for(SearchGenerationConstructionStage::SourceByteAcquisition)?;
        let expected = content_generation_digest(&self.acquisition)?;
        if self.content_generation_digest != expected {
            return Err("content search generation digest drift".to_owned());
        }
        Ok(())
    }

    #[must_use]
    pub fn identity(&self) -> &SearchGenerationIdentity {
        &self.acquisition.identity
    }
}

fn content_generation_digest(acquisition: &SearchGenerationStageReceipt) -> Result<String, String> {
    let bytes = serde_json::to_vec(acquisition)
        .map_err(|error| format!("encode content search generation receipt: {error}"))?;
    Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
}

pub fn canonical_blake3_digest(digest: &str) -> Result<String, String> {
    let value = digest
        .strip_prefix("blake3-256:")
        .or_else(|| digest.strip_prefix("blake3:"))
        .unwrap_or(digest);
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("value is not a BLAKE3 digest".to_owned());
    }
    Ok(format!("blake3-256:{value}"))
}

fn validate_digest(field: &str, digest: &str) -> Result<(), String> {
    if canonical_blake3_digest(digest).as_deref() != Ok(digest) {
        return Err(format!("{field} is not a canonical BLAKE3 digest"));
    }
    Ok(())
}
