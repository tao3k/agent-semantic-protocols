// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

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

enum RuntimeOwnerProjectionExecutor {
    Resident(ProviderRuntimeActorClient),
}

pub async fn prepare_runtime_server_owner_projection_with_resident_runtime_async(
    runtime: ProviderRuntimeActorClient,
    project_root: PathBuf,
    workspace_identity: String,
    owner_path: String,
    snapshot: RuntimeProviderProjection,
) -> Result<crate::runtime_server_workspace::WorkspaceOwnerProjection, String> {
    prepare_runtime_server_owner_projection_async(
        RuntimeOwnerProjectionExecutor::Resident(runtime),
        project_root,
        workspace_identity,
        owner_path,
        snapshot,
    )
    .await
}

async fn prepare_runtime_server_owner_projection_async(
    executor: RuntimeOwnerProjectionExecutor,
    project_root: PathBuf,
    workspace_identity: String,
    owner_path: String,
    snapshot: RuntimeProviderProjection,
) -> Result<crate::runtime_server_workspace::WorkspaceOwnerProjection, String> {
    let mut providers = snapshot.providers.iter().filter(|provider| {
        provider.runtime_operation("projection-batch").is_some()
            && provider
                .source_extensions
                .iter()
                .any(|extension| owner_path.ends_with(extension.as_str()))
    });
    let provider = providers.next().ok_or_else(|| {
        format!("runtime owner projection has no registered provider: ownerPath={owner_path}")
    })?;
    if let Some(ambiguous) = providers.next() {
        return Err(format!(
            "runtime owner projection provider ownership is ambiguous: ownerPath={owner_path} providers={},{}",
            provider.provider_id, ambiguous.provider_id
        ));
    }
    let authority = agent_semantic_search::ResidentSearchAuthority {
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
    };
    let source_path = agent_semantic_client_core::scoped_child_path(&project_root, &owner_path)
        .ok_or_else(|| {
            format!("runtime owner projection escaped workspace: ownerPath={owner_path}")
        })?;
    if !source_path.is_file() {
        return Err(format!(
            "runtime owner projection source is unavailable: ownerPath={owner_path}"
        ));
    }
    let files = vec![crate::ClientDbSourceIndexScopeFile {
        path: source_path,
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
        projection_coverage: crate::ClientDbSourceIndexProjectionCoverage::NotDeclared,
        projection_diagnostic: None,
        selector_receipts: Vec::new(),
        relations: Vec::new(),
    }];
    let registry = snapshot.evidence(&project_root);
    let (_, _, _, source_blobs, auxiliary_owners) =
        crate::server_source_index::async_snapshot::source_index_snapshot_from_files_async(
            &project_root,
            &files,
            &registry,
            &snapshot,
        )
        .await?;
    let projected = match executor {
        RuntimeOwnerProjectionExecutor::Resident(runtime) => {
            crate::server_source_index::projection::project_generation_with_resident_runtime(
                &runtime,
                &project_root,
                &workspace_identity,
                &snapshot,
                &files,
                &source_blobs,
                &auxiliary_owners,
            )
            .await?
        }
    };
    let projected = projected
        .into_iter()
        .next()
        .ok_or_else(|| "runtime owner projection omitted the target owner".to_owned())?;
    let bytes = source_blobs
        .iter()
        .find_map(|(path, bytes)| (path == owner_path).then(|| bytes.to_vec()))
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

pub async fn prepare_runtime_server_workspace_generation_with_runtime_service_async(
    runtime: crate::runtime_search_service::RuntimeSearchServiceHandle,
    project_id: String,
    workspace_id: String,
    project_root: PathBuf,
    snapshot: RuntimeProviderProjection,
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
