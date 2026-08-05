use std::path::PathBuf;
use std::time::Instant;

use agent_semantic_client_core::ProviderRegistrySnapshot;

use super::collect::{SourceIndexCollectionScope, collect_source_index_scope_async};
use super::generation_build::{SourceIndexGenerationRefresh, SourceIndexRefreshContext};

pub async fn prepare_runtime_server_owner_projection_with_registry_async(
    project_root: PathBuf,
    workspace_identity: String,
    owner_path: String,
    language_id: String,
    snapshot: ProviderRegistrySnapshot,
) -> Result<agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot, String> {
    let provider = snapshot
        .providers
        .iter()
        .find(|provider| {
            provider.language_id.as_str() == language_id
                && provider.language_projection.is_some()
                && provider
                    .source_extensions
                    .iter()
                    .any(|extension| owner_path.ends_with(extension.as_str()))
        })
        .ok_or_else(|| {
            format!(
                "runtime owner projection has no registered provider: languageId={language_id} ownerPath={owner_path}"
            )
        })?;
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
    let (_, _, _, source_blobs) = super::async_snapshot::source_index_snapshot_from_files_async(
        &project_root,
        &files,
        &registry,
    )
    .await?;
    let projected = super::projection::project_generation(
        &project_root,
        &workspace_identity,
        &snapshot,
        &files,
        &source_blobs,
    )
    .await?;
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
            content_digest: format!("blake3-256:{}", blake3::hash(&bytes).to_hex()),
            bytes,
            selectors,
        },
    )
}

pub async fn prepare_runtime_server_workspace_generation_async(
    project_root: PathBuf,
) -> Result<agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuild, String> {
    let snapshot = ProviderRegistrySnapshot::load(&project_root)?;
    prepare_runtime_server_workspace_generation_with_registry_async(project_root, snapshot).await
}

pub async fn prepare_runtime_server_workspace_generation_with_registry_async(
    project_root: PathBuf,
    snapshot: ProviderRegistrySnapshot,
) -> Result<agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuild, String> {
    let trace_started = Instant::now();
    let context = SourceIndexRefreshContext::resolve(&project_root)?;
    trace("context-resolved", trace_started);
    trace("provider-registry-admitted", trace_started);
    let registry = snapshot.evidence(&project_root);
    let collection = collect_source_index_scope_async(
        &project_root,
        &snapshot,
        &SourceIndexCollectionScope::CompleteGeneration,
    )
    .await?;
    trace("scope-files-collected", trace_started);
    context
        .prepare_generation_async(SourceIndexGenerationRefresh {
            index_root: &project_root,
            files: &collection.files,
            project_resolutions: &collection.project_resolutions,
            candidate: &collection.candidate,
            registry: &registry,
            provider_registry: &snapshot,
        })
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
