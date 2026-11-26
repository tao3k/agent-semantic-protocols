// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Closed-loop HTTP AST patching (`POST /v1/ast/patch`) routed through
//! [`crate::AspForwardingGateway`]'s supervised native compiler providers:
//! `locator -> ast-inspect -> lossless span splice -> ast-verify oracle calibration -> re-inspect -> fresh Open EDB batch`.

use agent_semantic_search::WorkspaceOpenEdbIngestBatch;
use serde::{Deserialize, Serialize};

use crate::calibration::{CalibrationRequest, calibrate_with_oracle};
use crate::open_edb_projection::project_inspection_to_open_edb_batch;
use crate::provider_port::{AstInspectRequest, AstProviderPort};
use crate::span_overlay::{
    SpanReplacement, blake3_source_digest, compute_op_span_edits, render_compact_unified_diff,
    splice_lossless,
};
use crate::types::{
    AstCalibrationOutcome, AstEngineError, AstLanguageId, AstNodeLocator, AstPatchOp,
    AstPatchStatus, AstProviderId, AstWorkspacePath, Blake3SourceDigest, ByteSpan,
    CalibrationReceipt, TypedHoleDiagnostic,
};

/// Schema ID for the forwarded HTTP AST patch response.
pub const AST_PATCH_FORWARDING_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.ast-patch-forwarding-response";

/// Closed-loop HTTP AST patch request accepted by `POST /v1/ast/patch`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardedAstPatchRequest {
    pub path: String,
    pub source_utf8: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language_override: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_preimage_hash: Option<Blake3SourceDigest>,
    pub operation: AstPatchOp,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default = "default_auto_calibrate")]
    pub auto_calibrate: bool,
}

const fn default_auto_calibrate() -> bool {
    true
}

/// Closed-loop HTTP AST patch response returned by `POST /v1/ast/patch`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardedAstPatchResponse {
    pub schema_id: String,
    pub schema_version: String,
    pub status: AstPatchStatus,
    pub path: AstWorkspacePath,
    pub language_id: AstLanguageId,
    pub provider_id: AstProviderId,
    pub preimage_hash: Blake3SourceDigest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postimage_hash: Option<Blake3SourceDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patched_source_utf8: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unified_diff: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration: Option<CalibrationReceipt>,
    pub reparse_verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_hole: Option<TypedHoleDiagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_edb_batch: Option<WorkspaceOpenEdbIngestBatch>,
}

fn combined_edited_span(edits: &[SpanReplacement]) -> ByteSpan {
    let Some(first) = edits.first() else {
        return ByteSpan { start: 0, end: 0 };
    };
    let mut start = first.span.start;
    let mut end = first.span.start + first.replacement.len();
    for edit in edits.iter().skip(1) {
        start = start.min(edit.span.start);
        end = end.max(edit.span.start + edit.replacement.len());
    }
    ByteSpan { start, end }
}

fn operation_locator(op: &AstPatchOp) -> Option<AstNodeLocator> {
    match op {
        AstPatchOp::ReplaceItem { target, .. }
        | AstPatchOp::AppendToBlock { target, .. }
        | AstPatchOp::InsertBeforeNode { target, .. }
        | AstPatchOp::InsertAfterNode { target, .. }
        | AstPatchOp::RemoveNode { target } => Some(target.clone()),
        AstPatchOp::InsertImport { .. } | AstPatchOp::ApplySpanEdits { .. } => None,
    }
}

/// Execute a pure in-memory lossless AST splice + Frontier 2 oracle calibration
/// + re-inspection + Open EDB projection against one resolved native provider.
pub(crate) fn apply_forwarded_patch_on_provider(
    provider: &dyn AstProviderPort,
    path: AstWorkspacePath,
    language_id: AstLanguageId,
    provider_id: AstProviderId,
    request: &ForwardedAstPatchRequest,
) -> Result<ForwardedAstPatchResponse, AstEngineError> {
    let preimage_hash = blake3_source_digest(request.source_utf8.as_bytes());
    if let Some(expected) = &request.expected_preimage_hash
        && expected != &preimage_hash
    {
        return Err(AstEngineError::PreimageMismatch {
            expected: expected.as_str().to_owned(),
            actual: preimage_hash.as_str().to_owned(),
        });
    }

    let inspected = provider.inspect(&AstInspectRequest {
        path: path.clone(),
        source_utf8: request.source_utf8.clone(),
        source_hash: preimage_hash.clone(),
    })?;
    let edits = compute_op_span_edits(&request.source_utf8, &inspected.nodes, &request.operation)?;
    let edited_span = combined_edited_span(&edits);
    let raw_spliced = splice_lossless(&request.source_utf8, &edits)?;

    let cal_outcome = calibrate_with_oracle(
        provider,
        &CalibrationRequest {
            path: &path,
            preimage_hash: &preimage_hash,
            raw_source: &raw_spliced,
            edited_span,
            locator: operation_locator(&request.operation),
            allow_auto_calibrate: request.auto_calibrate,
        },
    )?;

    let Some(final_source) = cal_outcome.calibrated_source else {
        return Ok(ForwardedAstPatchResponse {
            schema_id: AST_PATCH_FORWARDING_RESPONSE_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            status: AstPatchStatus::RejectedFailClosed,
            path,
            language_id,
            provider_id,
            preimage_hash,
            postimage_hash: None,
            patched_source_utf8: None,
            unified_diff: None,
            calibration: Some(cal_outcome.receipt),
            reparse_verified: false,
            typed_hole: cal_outcome.typed_hole,
            open_edb_batch: None,
        });
    };

    let postimage_hash = blake3_source_digest(final_source.as_bytes());
    let final_inspect = provider.inspect(&AstInspectRequest {
        path: path.clone(),
        source_utf8: final_source.clone(),
        source_hash: postimage_hash.clone(),
    })?;
    if !final_inspect.accepted {
        return Err(AstEngineError::ProviderError(
            "final re-parse verification failed after AST splice".to_owned(),
        ));
    }

    let diff = render_compact_unified_diff(path.as_str(), &request.source_utf8, &final_source);
    let open_edb_batch = project_inspection_to_open_edb_batch(
        &path,
        &language_id,
        &provider_id,
        &final_inspect.nodes,
        &final_inspect.edges,
    );
    let status = if request.dry_run {
        AstPatchStatus::Verified
    } else if cal_outcome.receipt.outcome == AstCalibrationOutcome::CalibratedUniqueMinCost {
        AstPatchStatus::CalibratedAndApplied
    } else {
        AstPatchStatus::Applied
    };

    Ok(ForwardedAstPatchResponse {
        schema_id: AST_PATCH_FORWARDING_RESPONSE_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        status,
        path,
        language_id,
        provider_id,
        preimage_hash,
        postimage_hash: Some(postimage_hash),
        patched_source_utf8: Some(final_source),
        unified_diff: (!diff.is_empty()).then_some(diff),
        calibration: Some(cal_outcome.receipt),
        reparse_verified: true,
        typed_hole: None,
        open_edb_batch: Some(open_edb_batch),
    })
}
