// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::PathBuf;

use agent_semantic_provider_transport::ProviderRuntimeActorClient;
use std::time::Instant;

use agent_semantic_client_core::RuntimeProviderProjection;

use crate::server_source_index::collect::SourceIndexCollectionScope;
use crate::server_source_index::generation_build::{
    SourceIndexGenerationRefresh, SourceIndexRefreshContext,
};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BaseGenerationBuildTimingReceipt {
    schema_id: &'static str,
    schema_version: &'static str,
    project_id: String,
    workspace_id: String,
    inventory_micros: u64,
    snapshot_merkle_materialization_micros: u64,
    content_receipt_micros: u64,
    total_micros: u64,
    provider_process_count: u8,
    provider_rpc_count: u8,
}

fn elapsed_micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "../../tests/unit/source_index_resident_projection.rs"]
mod tests;

/// Project generation-bound candidate bytes without reopening workspace files.
pub async fn prepare_runtime_server_resident_owner_projections_async(
    runtime: Option<ProviderRuntimeActorClient>,
    project_root: PathBuf,
    workspace_identity: String,
    mut owners: Vec<crate::runtime_server_workspace::WorkspaceOwnerSnapshot>,
    auxiliary_inputs: Vec<crate::runtime_server_workspace::WorkspaceAuxiliaryOwnerSnapshot>,
    snapshot: RuntimeProviderProjection,
    parser_artifact_root: PathBuf,
) -> Result<Vec<crate::runtime_server_workspace::WorkspaceOwnerProjection>, String> {
    let total_started = Instant::now();
    owners.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    owners.dedup_by(|left, right| left.owner_path == right.owner_path);
    let Some(first) = owners.first() else {
        return Ok(Vec::new());
    };
    let mut providers = snapshot.providers.iter().filter(|provider| {
        provider.runtime_operation("projection-batch").is_some()
            && provider
                .source_extensions
                .iter()
                .any(|extension| first.owner_path.ends_with(extension.as_str()))
    });
    let provider = providers.next().ok_or_else(|| {
        format!(
            "runtime owner projection has no registered provider: ownerPath={}",
            first.owner_path
        )
    })?;
    if let Some(ambiguous) = providers.next() {
        return Err(format!(
            "runtime owner projection provider ownership is ambiguous: ownerPath={} providers={},{}",
            first.owner_path, provider.provider_id, ambiguous.provider_id
        ));
    }
    let authority = agent_semantic_search::ResidentSearchAuthority {
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
    };
    for owner in &owners {
        if owner.authority.as_ref() != Some(&authority)
            || !provider
                .source_extensions
                .iter()
                .any(|extension| owner.owner_path.ends_with(extension.as_str()))
        {
            return Err(format!(
                "runtime resident owner escaped provider authority: ownerPath={} providerId={}",
                owner.owner_path, provider.provider_id
            ));
        }
        let digest = format!("blake3-256:{}", blake3::hash(&owner.bytes).to_hex());
        if digest != owner.content_digest {
            return Err(format!(
                "runtime resident owner content digest drift: ownerPath={}",
                owner.owner_path
            ));
        }
    }
    let owner_paths = owners
        .iter()
        .map(|owner| owner.owner_path.clone())
        .collect::<Vec<_>>();
    let files = owners
        .iter()
        .map(|owner| {
            let path =
                agent_semantic_client_core::scoped_child_path(&project_root, &owner.owner_path)
                    .ok_or_else(|| {
                        format!(
                            "runtime resident owner escaped workspace: ownerPath={}",
                            owner.owner_path
                        )
                    })?;
            Ok(crate::ClientDbSourceIndexScopeFile {
                path,
                language_id: provider.language_id.clone(),
                provider_id: provider.provider_id.clone(),
                projection_coverage: crate::ClientDbSourceIndexProjectionCoverage::NotDeclared,
                projection_diagnostic: None,
                selector_receipts: Vec::new(),
                relations: Vec::new(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let source_blobs =
        crate::ClientDbSourceIndexSourceBlobs::from_normalized(owners.iter().map(|owner| {
            (
                crate::ClientDbSourceIndexPath::new(owner.owner_path.clone()),
                owner.bytes.clone(),
            )
        }));
    let config_files = provider
        .config_files
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    let mut provider_auxiliary = Vec::new();
    for auxiliary in auxiliary_inputs {
        let file_name = std::path::Path::new(&auxiliary.owner_path)
            .file_name()
            .and_then(|name| name.to_str());
        if file_name.is_none_or(|name| !config_files.contains(&name.to_owned()))
            || !owner_paths.iter().any(|owner_path| {
                crate::server_source_index::projection::auxiliary_owner_applies_to_source(
                    &auxiliary.owner_path,
                    owner_path,
                )
            })
        {
            continue;
        }
        let digest = format!("blake3-256:{}", blake3::hash(&auxiliary.bytes).to_hex());
        if digest != auxiliary.content_digest {
            return Err(format!(
                "runtime resident auxiliary content digest drift: ownerPath={}",
                auxiliary.owner_path
            ));
        }
        provider_auxiliary.push(
            agent_semantic_provider_transport::projection_batch::ProviderProjectionOwner {
                owner_path: auxiliary.owner_path,
                source_leaf_digest: digest
                    .strip_prefix("blake3-256:")
                    .expect("constructed digest prefix")
                    .to_owned(),
                source_bytes: auxiliary.bytes,
            },
        );
    }
    provider_auxiliary.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    let auxiliary_owners = std::collections::BTreeMap::from([(
        provider.provider_id.as_str().to_owned(),
        provider_auxiliary,
    )]);
    let resident_source_bytes = owners.iter().map(|owner| owner.bytes.len()).sum::<usize>();
    let resident_auxiliary_bytes = auxiliary_owners
        .values()
        .flatten()
        .map(|owner| owner.source_bytes.len())
        .sum::<usize>();
    eprintln!(
        "[runtime-owner-resident-input-timing] providerId={} ownerCount={} auxiliaryOwnerCount={} sourceFilesystemReadCount=0 auxiliaryFilesystemReadCount=0 residentSourceBytes={} residentAuxiliaryBytes={} inputPreparationMicros={}",
        provider.provider_id,
        owners.len(),
        auxiliary_owners.values().map(Vec::len).sum::<usize>(),
        resident_source_bytes,
        resident_auxiliary_bytes,
        elapsed_micros(total_started),
    );
    let projection_started = Instant::now();
    let projected = crate::server_source_index::projection::project_generation_with_resident_runtime_and_artifact_store(
        runtime.as_ref(),
        &project_root,
        &workspace_identity,
        &snapshot,
        &files,
        &source_blobs,
        &auxiliary_owners,
        &parser_artifact_root,
    )
    .await?;
    let projection_micros = elapsed_micros(projection_started);
    let projected_by_path = projected
        .into_iter()
        .map(|projected| (projected.path.clone(), projected))
        .collect::<std::collections::BTreeMap<_, _>>();
    let result = owner_paths
        .into_iter()
        .map(|owner_path| {
            let source_path =
                agent_semantic_client_core::scoped_child_path(&project_root, &owner_path)
                    .ok_or_else(|| {
                        format!("runtime resident owner escaped workspace: ownerPath={owner_path}")
                    })?;
            let projected = projected_by_path
                .get(&source_path)
                .cloned()
                .ok_or_else(|| {
                    format!("runtime owner projection omitted target owner: ownerPath={owner_path}")
                })?;
            owner_projection_from_projected_file(
                projected,
                &source_blobs,
                owner_path,
                authority.clone(),
            )
        })
        .collect::<Result<Vec<_>, String>>();
    eprintln!(
        "[runtime-owner-projection-pipeline-timing] providerId={} ownerCount={} sourceFilesystemReadCount=0 auxiliaryFilesystemReadCount=0 residentSourceBytes={} residentAuxiliaryBytes={} projectionMicros={} totalMicros={}",
        provider.provider_id,
        owners.len(),
        resident_source_bytes,
        resident_auxiliary_bytes,
        projection_micros,
        elapsed_micros(total_started),
    );
    result
}

fn owner_projection_from_projected_file(
    projected: crate::ClientDbSourceIndexScopeFile,
    source_blobs: &crate::ClientDbSourceIndexSourceBlobs,
    owner_path: String,
    authority: agent_semantic_search::ResidentSearchAuthority,
) -> Result<crate::runtime_server_workspace::WorkspaceOwnerProjection, String> {
    let bytes = source_blobs
        .iter()
        .find_map(|(path, bytes)| (path == owner_path.as_str()).then(|| bytes.to_vec()))
        .ok_or_else(|| {
            format!("runtime owner projection omitted source bytes: ownerPath={owner_path}")
        })?;
    let content_digest = format!("blake3-256:{}", blake3::hash(&bytes).to_hex());
    let native_syntax_diagnostic = projected.projection_diagnostic.map(|diagnostic| {
        agent_semantic_search::NativeSyntaxDiagnostic {
            owner_path: owner_path.clone(),
            content_digest: content_digest.clone(),
            reason_kind: diagnostic.reason_kind,
            message: diagnostic.message,
        }
    });
    let mut selectors = projected
        .selector_receipts
        .into_iter()
        .map(|selector| {
            let proof = &selector.projection_record.proof;
            if proof.owner_path() != owner_path {
                return Err(format!(
                    "runtime owner projection selector owner drift: expected={owner_path} actual={}",
                    proof.owner_path()
                ));
            }
            let byte_start = usize::try_from(selector.projection_record.source_byte_range.start)
                .map_err(|_| "runtime owner projection byte start overflow".to_owned())?;
            let byte_end = usize::try_from(selector.projection_record.source_byte_range.end)
                .map_err(|_| "runtime owner projection byte end overflow".to_owned())?;
            if bytes.get(byte_start..byte_end)
                != Some(selector.projection_record.projection_payload.as_slice())
            {
                return Err(format!(
                    "runtime owner projection payload drift: selector={}",
                    proof.structural_selector()
                ));
            }
            let structural_selector = proof.structural_selector().to_owned();
            let mut query_keys = selector
                .query_keys
                .into_iter()
                .map(|key| key.as_str().to_owned())
                .collect::<Vec<_>>();
            query_keys.sort();
            query_keys.dedup();
            Ok(crate::runtime_server_workspace::WorkspaceSelectorSnapshot {
                selector: structural_selector,
                byte_start,
                byte_end,
                query_keys,
                derived_projections: selector.derived_projections,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    selectors.sort_by(|left, right| left.selector.cmp(&right.selector));
    let relations = projected
        .relations
        .into_iter()
        .map(|relation| crate::ClientDbSourceIndexOwnedRelation {
            owner_path: crate::ClientDbSourceIndexPath::new(owner_path.clone()),
            relation,
        })
        .collect();
    Ok(crate::runtime_server_workspace::WorkspaceOwnerProjection {
        owner: crate::runtime_server_workspace::WorkspaceOwnerSnapshot {
            owner_path,
            authority: Some(authority),
            content_digest,
            native_syntax_diagnostic,
            bytes,
            selectors,
        },
        relations,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the rebuild boundary binds all generation identities, budgets, and cancellation authority"
)]
pub async fn prepare_runtime_server_workspace_generation_with_runtime_service_async(
    runtime: crate::runtime_search_service::RuntimeSearchServiceHandle,
    project_id: String,
    workspace_id: String,
    project_root: PathBuf,
    snapshot: RuntimeProviderProjection,
    recovery_execution: super::generation_recovery::SourceIndexRecoveryExecution,
    collection_scope: SourceIndexCollectionScope,
    candidate: crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
    inventory: Vec<String>,
    cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
) -> Result<crate::runtime_server_admission::WorkspaceGenerationCandidateBuild, String> {
    let trace_started = Instant::now();
    let context = SourceIndexRefreshContext::resolve(&project_root)?;
    trace("context-resolved", trace_started);
    trace("provider-registry-admitted", trace_started);
    let registry = snapshot.evidence(&project_root);
    let inventory_started = Instant::now();
    let collection =
        crate::server_source_index::collect::collect_source_index_scope_from_inventory(
            &project_root,
            &snapshot,
            &collection_scope,
            &inventory,
            candidate,
        )?;
    let inventory_micros = elapsed_micros(inventory_started);
    trace("scope-files-collected", trace_started);
    let SourceIndexCollectionScope::CompleteGeneration = &collection_scope;
    let snapshot_started = Instant::now();
    let prepared = context
        .prepare_generation_with_runtime_service_async(
            &runtime,
            SourceIndexGenerationRefresh {
                recovery_execution: Some(&recovery_execution),
                changed_owner_paths: None,
                replacement_authority: None,
                index_root: &project_root,
                files: &collection.files,
                project_resolutions: &collection.project_resolutions,
                candidate: &collection.candidate,
                registry: &registry,
                provider_registry: &snapshot,
            },
            cancellation.clone(),
        )
        .await?;
    let snapshot_merkle_materialization_micros = elapsed_micros(snapshot_started);
    let mut build = prepared.into_runtime_server_build();
    let content_receipt_started = Instant::now();
    finalize_content_search_generation(
        &runtime,
        &mut build,
        &project_id,
        &workspace_id,
        cancellation,
    )
    .await?;
    let content_receipt_micros = elapsed_micros(content_receipt_started);
    eprintln!(
        "[base-generation-build-timing] {}",
        serde_json::to_string(&BaseGenerationBuildTimingReceipt {
            schema_id: "agent.semantic-protocols.base-generation-build-timing-receipt",
            schema_version: "1",
            project_id,
            workspace_id,
            inventory_micros,
            snapshot_merkle_materialization_micros,
            content_receipt_micros,
            total_micros: elapsed_micros(trace_started),
            provider_process_count: 0,
            provider_rpc_count: 0,
        })
        .map_err(|error| format!("encode base generation timing receipt: {error}"))?
    );
    Ok(build)
}

async fn finalize_content_search_generation(
    _runtime: &crate::runtime_search_service::RuntimeSearchServiceHandle,
    build: &mut crate::runtime_server_admission::WorkspaceGenerationCandidateBuild,
    project_id: &str,
    workspace_id: &str,
    cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
) -> Result<(), String> {
    if cancellation.is_cancelled() {
        return Err("search generation construction cancelled".to_owned());
    }
    let materialization = &mut build.materialization;
    if materialization.workspace_identity != workspace_id {
        return Err("search generation workspaceId differs from its materialization".to_owned());
    }
    let identity = agent_semantic_search::SearchGenerationIdentity {
        project_id: project_id.to_owned(),
        workspace_id: workspace_id.to_owned(),
        source_root_digest: agent_semantic_search::canonical_blake3_digest(
            &materialization.source_snapshot.root_digest,
        )?,
        provider_digest: agent_semantic_search::canonical_blake3_digest(
            &materialization.source_snapshot.provider_digest,
        )?,
        schema_digest: agent_semantic_search::canonical_blake3_digest(
            &agent_semantic_content_identity::project_resolution_schema_digest(),
        )?,
        generation_candidate_digest: agent_semantic_search::canonical_blake3_digest(
            &build.candidate.candidate_generation.digest,
        )?,
    };
    identity.validate()?;
    let source_owners = materialization
        .owners
        .iter()
        .map(|owner| {
            (
                owner.owner_path.clone(),
                owner.content_digest.clone(),
                owner.bytes.len(),
            )
        })
        .collect::<Vec<_>>();
    let acquisition = crate::runtime_server_runtime::RuntimeServerOwnedTask::spawn_blocking(
        "content-search-generation-byte-acquisition",
        move || {
            agent_semantic_search::build_source_byte_acquisition_stage(
                identity,
                source_owners
                    .iter()
                    .map(|(owner_path, content_digest, byte_len)| {
                        agent_semantic_search::SourceByteOwner {
                            owner_path,
                            content_digest,
                            byte_len: *byte_len,
                        }
                    }),
            )
        },
    )
    .join()
    .await??;
    let receipt = agent_semantic_search::ContentSearchGenerationReceipt::new(acquisition)?;
    materialization.attach_content_search_generation(receipt)
}

fn trace(stage: &str, started: Instant) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-trace] stage={} elapsedMs={}",
            stage,
            started.elapsed().as_millis()
        );
    }
}
