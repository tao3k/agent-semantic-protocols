// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pure Rust Backend Runtime for transactional lossless AST inspection,
//! Frontier 2 oracle-verified syntax calibration, atomic patching, and
//! single-tree Open EDB fact projection (`AstEngine`).
//!
//! Coding Style policy, `:gerbil-ascent` rules, and elevation decisions live
//! exclusively in the Scheme extension plane (`scheme/ast-style/`), which
//! consumes [`AstFileInspection`] facts from [`AstEngine::inspect_file`] and
//! submits typed [`AstPatchOp`]s back into [`AstEngine::apply_patch`].

use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_search::{
    WorkspaceFileDelta, WorkspaceOpenEdbIngestBatch, WorkspaceSearchIndex,
};

use crate::calibration::{CalibrationRequest, calibrate_with_oracle};
use crate::open_edb_projection::project_nodes_to_open_edb_batch;
use crate::provider_port::{AstInspectRequest, AstProviderPort};
use crate::registry::AstProviderRegistry;
use crate::span_overlay::{
    SpanReplacement, blake3_source_digest, compute_op_span_edits, render_compact_unified_diff,
    splice_lossless,
};
use crate::types::{
    AstCalibrationOutcome, AstEngineError, AstLanguageId, AstNodeLocator, AstNodeSpanRecord,
    AstPatchOp, AstPatchReceipt, AstPatchRequest, AstPatchStatus, AstProviderId,
    AstSyntaxDiagnostic, AstWorkspacePath, Blake3SourceDigest, ByteSpan,
};

/// Authoritative native-compiler AST inspection & Open EDB projection for one
/// workspace file, ready to be queried by Scheme `:gerbil-ascent` policy packs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AstFileInspection {
    pub path: AstWorkspacePath,
    pub language_id: AstLanguageId,
    pub provider_id: AstProviderId,
    pub source_hash: Blake3SourceDigest,
    pub accepted: bool,
    pub nodes: Vec<AstNodeSpanRecord>,
    pub diagnostics: Vec<AstSyntaxDiagnostic>,
    pub open_edb_batch: WorkspaceOpenEdbIngestBatch,
}

/// Pure, language-agnostic AI-Native Language Server AST Backend Runtime.
pub struct AstEngine {
    workspace_root: PathBuf,
    providers: AstProviderRegistry,
}

impl AstEngine {
    /// Construct an [`AstEngine`] anchored at `workspace_root` with an empty provider registry.
    #[must_use]
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            providers: AstProviderRegistry::new(),
        }
    }

    /// Construct an [`AstEngine`] with a pre-populated [`AstProviderRegistry`].
    #[must_use]
    pub fn with_registry(workspace_root: PathBuf, providers: AstProviderRegistry) -> Self {
        Self {
            workspace_root,
            providers,
        }
    }

    /// Borrow the underlying [`AstProviderRegistry`].
    #[must_use]
    pub const fn providers(&self) -> &AstProviderRegistry {
        &self.providers
    }

    /// Mutably borrow the underlying [`AstProviderRegistry`] to register or unregister providers.
    pub const fn providers_mut(&mut self) -> &mut AstProviderRegistry {
        &mut self.providers
    }

    /// Inspect `path` through its registered native compiler provider and emit
    /// both authoritative UTF-8 [`AstNodeSpanRecord`]s and a `semantic-open-edb.v1`
    /// fact batch for Scheme `:gerbil-ascent` Coding Style & search reasoning.
    pub fn inspect_file(
        &self,
        path: &AstWorkspacePath,
        language_override: Option<&AstLanguageId>,
    ) -> Result<AstFileInspection, AstEngineError> {
        let resolved = self.providers.resolve(path, language_override)?;
        let abs_path = self.workspace_root.join(path.as_str());
        let source = fs::read_to_string(&abs_path).map_err(|err| {
            AstEngineError::IoError(format!("read {}: {err}", abs_path.display()))
        })?;
        let source_hash = blake3_source_digest(source.as_bytes());
        let inspected = resolved.port.inspect(&AstInspectRequest {
            path: path.clone(),
            source_utf8: source,
            source_hash: source_hash.clone(),
        })?;
        let open_edb_batch = project_nodes_to_open_edb_batch(
            path,
            &resolved.descriptor.language_id,
            &resolved.descriptor.provider_id,
            &inspected.nodes,
        );
        Ok(AstFileInspection {
            path: path.clone(),
            language_id: resolved.descriptor.language_id,
            provider_id: resolved.descriptor.provider_id,
            source_hash,
            accepted: inspected.accepted,
            nodes: inspected.nodes,
            diagnostics: inspected.diagnostics,
            open_edb_batch,
        })
    }

    /// Execute one lossless AST patch transaction against disk (or dry-run),
    /// routing automatically through [`AstProviderRegistry`].
    pub fn apply_patch(
        &self,
        request: &AstPatchRequest,
    ) -> Result<AstPatchReceipt, AstEngineError> {
        self.apply_patch_inner(request, None)
    }

    /// Execute one lossless AST patch transaction and synchronously update a
    /// live [`WorkspaceSearchIndex`] + Open EDB in the same atomic step.
    pub fn apply_patch_with_index(
        &self,
        request: &AstPatchRequest,
        index: &mut WorkspaceSearchIndex,
    ) -> Result<AstPatchReceipt, AstEngineError> {
        self.apply_patch_inner(request, Some(index))
    }

    fn apply_patch_inner(
        &self,
        request: &AstPatchRequest,
        index: Option<&mut WorkspaceSearchIndex>,
    ) -> Result<AstPatchReceipt, AstEngineError> {
        let resolved = self
            .providers
            .resolve(&request.path, request.language_id.as_ref())?;
        let provider = resolved.port.as_ref();
        let abs_path = self.workspace_root.join(request.path.as_str());
        let original = fs::read_to_string(&abs_path).map_err(|err| {
            AstEngineError::IoError(format!("read {}: {err}", abs_path.display()))
        })?;
        let preimage_hash = blake3_source_digest(original.as_bytes());
        if let Some(expected) = &request.expected_preimage_hash
            && expected != &preimage_hash
        {
            return Err(AstEngineError::PreimageMismatch {
                expected: expected.as_str().to_owned(),
                actual: preimage_hash.as_str().to_owned(),
            });
        }

        let inspected = provider.inspect(&AstInspectRequest {
            path: request.path.clone(),
            source_utf8: original.clone(),
            source_hash: preimage_hash.clone(),
        })?;
        let edits = compute_op_span_edits(&original, &inspected.nodes, &request.operation)?;
        let edited_span = combined_edited_span(&edits);
        let raw_spliced = splice_lossless(&original, &edits)?;
        let cal_outcome = calibrate_with_oracle(
            provider,
            &CalibrationRequest {
                path: &request.path,
                preimage_hash: &preimage_hash,
                raw_source: &raw_spliced,
                edited_span,
                locator: operation_locator(&request.operation),
                allow_auto_calibrate: request.auto_calibrate,
            },
        )?;

        let Some(final_source) = cal_outcome.calibrated_source else {
            return Ok(AstPatchReceipt {
                status: AstPatchStatus::RejectedFailClosed,
                path: request.path.clone(),
                language_id: resolved.descriptor.language_id,
                provider_id: resolved.descriptor.provider_id,
                preimage_hash,
                postimage_hash: None,
                calibration: Some(cal_outcome.receipt),
                reparse_verified: false,
                typed_hole: cal_outcome.typed_hole,
                unified_diff: None,
                open_edb_ingested_entities: 0,
            });
        };

        let postimage_hash = blake3_source_digest(final_source.as_bytes());
        let final_inspect = provider.inspect(&AstInspectRequest {
            path: request.path.clone(),
            source_utf8: final_source.clone(),
            source_hash: postimage_hash.clone(),
        })?;
        if !final_inspect.accepted {
            return Err(AstEngineError::ProviderError(
                "final re-parse verification failed after AST splice".to_owned(),
            ));
        }

        let diff = render_compact_unified_diff(request.path.as_str(), &original, &final_source);
        let mut ingested = 0usize;
        if !request.dry_run {
            atomic_write_file(&abs_path, final_source.as_bytes())?;
            if let Some(live_index) = index {
                ingested = sync_open_edb(
                    live_index,
                    request,
                    &final_source,
                    provider,
                    &final_inspect.nodes,
                );
            }
        }

        let status = if request.dry_run {
            AstPatchStatus::Verified
        } else if cal_outcome.receipt.outcome == AstCalibrationOutcome::CalibratedUniqueMinCost {
            AstPatchStatus::CalibratedAndApplied
        } else {
            AstPatchStatus::Applied
        };

        Ok(AstPatchReceipt {
            status,
            path: request.path.clone(),
            language_id: resolved.descriptor.language_id,
            provider_id: resolved.descriptor.provider_id,
            preimage_hash,
            postimage_hash: Some(postimage_hash),
            calibration: Some(cal_outcome.receipt),
            reparse_verified: true,
            typed_hole: None,
            unified_diff: (!diff.is_empty()).then_some(diff),
            open_edb_ingested_entities: ingested,
        })
    }
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

fn atomic_write_file(target: &Path, bytes: &[u8]) -> Result<(), AstEngineError> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let file_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("ast-patch");
    let tmp = parent.join(format!(".{file_name}.asp-ast-tmp"));
    fs::write(&tmp, bytes)
        .map_err(|err| AstEngineError::IoError(format!("write temp {}: {err}", tmp.display())))?;
    fs::rename(&tmp, target).map_err(|err| {
        let _ = fs::remove_file(&tmp);
        AstEngineError::IoError(format!("commit {}: {err}", target.display()))
    })
}

fn sync_open_edb(
    live_index: &mut WorkspaceSearchIndex,
    request: &AstPatchRequest,
    final_source: &str,
    provider: &dyn AstProviderPort,
    nodes: &[AstNodeSpanRecord],
) -> usize {
    let _ = live_index.apply_file_delta(WorkspaceFileDelta {
        upserts: vec![(
            request.path.as_str().to_owned(),
            final_source.as_bytes().to_vec(),
        )],
        deletes: Vec::new(),
    });
    let batch = project_nodes_to_open_edb_batch(
        &request.path,
        provider.language_id(),
        provider.provider_id(),
        nodes,
    );
    live_index.ingest_open_edb_facts(batch).ingested_entities
}
