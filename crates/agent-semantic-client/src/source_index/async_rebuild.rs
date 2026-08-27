use std::path::PathBuf;

use agent_semantic_provider_transport::ProviderRuntimeActorClient;
use std::time::Instant;

use agent_semantic_client_core::RuntimeProviderProjection;

use super::collect::SourceIndexCollectionScope;
use super::generation_build::{SourceIndexGenerationRefresh, SourceIndexRefreshContext};

enum RuntimeOwnerProjectionExecutor {
    Resident(ProviderRuntimeActorClient),
}

pub async fn prepare_runtime_server_owner_projection_with_resident_runtime_async(
    runtime: ProviderRuntimeActorClient,
    project_root: PathBuf,
    workspace_identity: String,
    owner_path: String,
    snapshot: RuntimeProviderProjection,
) -> Result<agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot, String> {
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
) -> Result<agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot, String> {
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
    let files = vec![agent_semantic_client_db::ClientDbSourceIndexScopeFile {
        path: source_path,
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
        projection_coverage:
            agent_semantic_client_db::ClientDbSourceIndexProjectionCoverage::NotDeclared,
        selector_receipts: Vec::new(),
        relations: Vec::new(),
    }];
    let registry = snapshot.evidence(&project_root);
    let (_, _, _, source_blobs, auxiliary_owners) =
        super::async_snapshot::source_index_snapshot_from_files_async(
            &project_root,
            &files,
            &registry,
            &snapshot,
        )
        .await?;
    let projected = match executor {
        RuntimeOwnerProjectionExecutor::Resident(runtime) => {
            super::projection::project_generation_with_resident_runtime(
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
            Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot {
                selector: proof.structural_selector().to_owned(),
                byte_start,
                byte_end,
                derived_projections: selector.derived_projections,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    selectors.sort_by(|left, right| left.selector.cmp(&right.selector));
    Ok(
        agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot {
            owner_path,
            authority: Some(authority),
            content_digest: format!("blake3-256:{}", blake3::hash(&bytes).to_hex()),
            bytes,
            selectors,
        },
    )
}

pub async fn prepare_runtime_server_workspace_generation_with_runtime_service_async(
    runtime: agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
    project_root: PathBuf,
    snapshot: RuntimeProviderProjection,
    collection_scope: SourceIndexCollectionScope,
    cancellation: agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation,
) -> Result<
    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateBuild,
    String,
> {
    let trace_started = Instant::now();
    let context = SourceIndexRefreshContext::resolve(&project_root)?;
    trace("context-resolved", trace_started);
    trace("provider-registry-admitted", trace_started);
    let registry = snapshot.evidence(&project_root);
    let collection = super::collect::collect_source_index_scope_with_runtime_service_async(
        &runtime,
        &project_root,
        &snapshot,
        &collection_scope,
        cancellation.clone(),
    )
    .await?;
    trace("scope-files-collected", trace_started);
    context
        .prepare_generation_with_runtime_service_async(
            &runtime,
            SourceIndexGenerationRefresh {
                changed_owner_paths: collection_scope.explicit_owner_paths(),
                index_root: &project_root,
                files: &collection.files,
                project_resolutions: &collection.project_resolutions,
                candidate: &collection.candidate,
                registry: &registry,
                provider_registry: &snapshot,
            },
            cancellation,
        )
        .await
        .map(|prepared| prepared.into_runtime_server_build())
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
