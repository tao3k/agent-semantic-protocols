// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeSet;
use std::path::Path;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use agent_semantic_artifacts::CatalogObservation;
use agent_semantic_artifacts::CleanupDisposition;
use agent_semantic_artifacts::CleanupPlan;
use agent_semantic_artifacts::CleanupSelection;
use agent_semantic_artifacts::ProjectBinding;
use agent_semantic_artifacts::RetainedObject;
use agent_semantic_artifacts::RetentionLease;
use agent_semantic_artifacts::RetentionObjectKind;
use agent_semantic_artifacts::StagedStateHomeRemoval;
use agent_semantic_artifacts::StateHomeLayout;
use agent_semantic_client_db::StateHomeCatalog;

use agent_semantic_runtime::state_core::ResolvedState;

use crate::cache_cli::clean_args::CacheCleanArgs;

pub(crate) async fn apply_cache_cleanup(
    project_root: &Path,
    args: &CacheCleanArgs,
    receipt_json: bool,
) -> Result<(), String> {
    let grace_period_ms = args.day.saturating_mul(24 * 60 * 60 * 1_000);
    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    let state = ResolvedState::resolve_with_state_home(project_root, state_home)?;
    let layout = StateHomeLayout::new(&state.state_home);
    let canonical_workspaces = layout.materialized_workspaces()?;
    let (catalog_generation, catalog_plan) =
        admit_cleanup_with_catalog(&state, &canonical_workspaces, grace_period_ms, args).await?;

    let selected_object_ids = catalog_plan
        .entries
        .iter()
        .filter(|entry| matches!(entry.disposition, CleanupDisposition::Delete { .. }))
        .map(|entry| entry.object.object_id.clone())
        .collect::<BTreeSet<_>>();
    let catalog = StateHomeCatalog::open(layout.catalog()).await?;
    let current_catalog_generation = catalog.generation().await?.get();
    if current_catalog_generation != catalog_generation {
        return Err(format!(
            "reasonKind=state-home-cleanup-catalog-generation-changed expected={catalog_generation} actual={current_catalog_generation}"
        ));
    }
    let mut staged_workspaces = Vec::new();
    for workspace in &canonical_workspaces {
        let object_id = canonical_workspace_object_id(&workspace.binding);
        if selected_object_ids.contains(&object_id) {
            match layout.stage_workspace_removal(workspace.binding.workspace.digest.as_str()) {
                Ok(staged) => staged_workspaces.push(staged),
                Err(error) => {
                    let rollback = rollback_staged_workspaces(staged_workspaces);
                    return Err(combine_cleanup_and_rollback_error(error, rollback));
                }
            }
        }
    }
    let canonical_deleted = staged_workspaces.len();
    let committed_catalog_generation = if selected_object_ids.is_empty() {
        catalog_generation
    } else {
        match catalog
            .delete_objects(
                agent_semantic_artifacts::CatalogGeneration::new(catalog_generation),
                &selected_object_ids,
            )
            .await
        {
            Ok(generation) => generation.get(),
            Err(error) => {
                let rollback = rollback_staged_workspaces(staged_workspaces);
                return Err(combine_cleanup_and_rollback_error(error, rollback));
            }
        }
    };
    let reap_errors = reap_staged_workspaces(staged_workspaces);
    if !reap_errors.is_empty() {
        return Err(format!(
            "reasonKind=state-home-cleanup-reap-pending catalogGeneration={} errors={}",
            committed_catalog_generation,
            reap_errors.join(" | ")
        ));
    }

    println!(
        "[asp-state-home-clean] status=applied retainedForDays={} catalogGeneration={} canonicalWorkspacesDeleted={} deletedBytes={}",
        args.day, committed_catalog_generation, canonical_deleted, catalog_plan.deleted_bytes,
    );
    if receipt_json {
        let receipt = serde_json::to_string(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.state-home-cleanup-receipt",
            "schemaVersion": 1,
            "state": "applied",
            "retainedForDays": args.day,
            "catalogGeneration": committed_catalog_generation,
            "canonicalWorkspacesDeleted": canonical_deleted,
            "catalogPlan": catalog_plan,
        }))
        .map_err(|error| format!("failed to serialize workspace cleanup receipt: {error}"))?;
        eprintln!("{receipt}");
    }
    Ok(())
}

fn now_ms() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock precedes Unix epoch: {error}"))?
        .as_millis()
        .try_into()
        .map_err(|_| "system timestamp exceeds u64".to_string())
}

fn rollback_staged_workspaces(mut staged: Vec<StagedStateHomeRemoval>) -> Result<(), String> {
    let mut failures = Vec::new();
    while let Some(workspace) = staged.pop() {
        if let Err(error) = workspace.rollback() {
            failures.push(error);
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join(" | "))
    }
}

fn reap_staged_workspaces(staged: Vec<StagedStateHomeRemoval>) -> Vec<String> {
    staged
        .into_iter()
        .filter_map(|workspace| workspace.commit().err())
        .collect()
}

fn combine_cleanup_and_rollback_error(
    cleanup_error: String,
    rollback: Result<(), String>,
) -> String {
    match rollback {
        Ok(()) => cleanup_error,
        Err(rollback_error) => format!(
            "{cleanup_error}; reasonKind=state-home-cleanup-rollback-failed error={rollback_error}"
        ),
    }
}

async fn admit_cleanup_with_catalog(
    state: &ResolvedState,
    canonical_workspaces: &[agent_semantic_artifacts::MaterializedWorkspaceState],
    grace_period_ms: u64,
    args: &CacheCleanArgs,
) -> Result<(u64, CleanupPlan), String> {
    let evaluated_at_ms = now_ms()?;
    let layout = StateHomeLayout::new(&state.state_home);
    let catalog = StateHomeCatalog::open(layout.catalog()).await?;
    let mut observations = Vec::with_capacity(canonical_workspaces.len());
    let active_binding = state.project_binding()?;
    for workspace in canonical_workspaces {
        let object_id = canonical_workspace_object_id(&workspace.binding);
        observations.push(CatalogObservation {
            binding: workspace.binding.clone(),
            object: RetainedObject {
                object_id: object_id.clone(),
                kind: RetentionObjectKind::Workspace,
                last_observed_at_ms: workspace.last_observed_at_ms,
                byte_count: workspace.byte_count,
            },
            leases: (workspace.binding.workspace.digest == active_binding.workspace.digest)
                .then(|| RetentionLease {
                    lease_id: format!("active-workspace:{object_id}"),
                    object_id,
                    owner: "runtime-workspace-resolution".to_string(),
                    expires_at_ms: None,
                })
                .into_iter()
                .collect(),
            observed_at_ms: evaluated_at_ms,
        });
    }
    let generation = if observations.is_empty() {
        catalog.generation().await?
    } else {
        catalog.observe_batch(&observations).await?.generation
    };
    let selection = cleanup_selection(args, &observations)?;
    let plan = catalog
        .plan_cleanup_selected(evaluated_at_ms, grace_period_ms, selection)
        .await?;
    Ok((generation.get(), plan))
}

fn cleanup_selection(
    args: &CacheCleanArgs,
    observations: &[CatalogObservation],
) -> Result<CleanupSelection, String> {
    if let Some(workspace_digest) = &args.workspace_digest {
        return Ok(CleanupSelection::WorkspaceDigest {
            workspace_digest: workspace_digest.clone(),
        });
    }
    if let Some(object_id) = &args.object_id {
        return Ok(CleanupSelection::ObjectId {
            object_id: object_id.clone(),
        });
    }
    let Some(workspace_root) = &args.workspace_root else {
        return Ok(CleanupSelection::All);
    };
    let canonical_root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.clone());
    let matches = observations
        .iter()
        .filter(|observation| observation.binding.workspace.canonical_root == canonical_root)
        .map(|observation| observation.binding.workspace.digest.to_string())
        .collect::<BTreeSet<_>>();
    match matches.len() {
        0 => Err(format!(
            "reasonKind=state-home-cleanup-selection-no-match workspaceRoot={}",
            canonical_root.display()
        )),
        1 => Ok(CleanupSelection::WorkspaceDigest {
            workspace_digest: matches.into_iter().next().expect("one workspace digest"),
        }),
        count => Err(format!(
            "reasonKind=state-home-cleanup-selection-ambiguous workspaceRoot={} matches={count}",
            canonical_root.display()
        )),
    }
}

fn canonical_workspace_object_id(binding: &ProjectBinding) -> String {
    format!("workspace:{}", binding.workspace.digest)
}
