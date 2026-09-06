// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

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
use agent_semantic_artifacts::RetiredStateRoot;
use agent_semantic_artifacts::StagedWorkspaceRetirement;
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
    let retired_state_roots = layout.retired_state_roots()?;
    let (catalog_generation, catalog_plan) =
        admit_cleanup_with_catalog(&state, &canonical_workspaces, grace_period_ms, args).await?;

    let selected_object_ids = catalog_plan
        .entries
        .iter()
        .filter_map(|entry| {
            matches!(entry.disposition, CleanupDisposition::Retire { .. })
                .then(|| entry.object.object_id.clone())
        })
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
            match layout.stage_workspace_retirement(workspace.binding.workspace.digest.as_str()) {
                Ok(staged) => staged_workspaces.push(staged),
                Err(error) => {
                    let rollback = rollback_staged_workspaces(staged_workspaces);
                    return Err(combine_cleanup_and_rollback_error(error, rollback));
                }
            }
        }
    }
    let evaluated_at_ms = now_ms()?;
    let selected_retired_roots = retired_state_roots
        .iter()
        .filter(|object| {
            evaluated_at_ms.saturating_sub(object.last_observed_at_ms) >= grace_period_ms
                && retired_state_root_selected(args, object)
        })
        .collect::<Vec<_>>();
    let retired_state_bytes = selected_retired_roots.iter().fold(0_u64, |total, object| {
        total.saturating_add(object.byte_count)
    });
    for object in &selected_retired_roots {
        match layout.stage_retired_state_root(object) {
            Ok(staged) => staged_workspaces.push(staged),
            Err(error) => {
                let rollback = rollback_staged_workspaces(staged_workspaces);
                return Err(combine_cleanup_and_rollback_error(error, rollback));
            }
        }
    }
    let canonical_retired = staged_workspaces.len();
    let retired_state_root_count = selected_retired_roots.len();
    let canonical_retired = canonical_retired.saturating_sub(retired_state_root_count);
    let committed_catalog_generation = if selected_object_ids.is_empty() {
        catalog_generation
    } else {
        match catalog
            .retire_objects(
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
        "[asp-state-home-clean] status=applied retainedForDays={} catalogGeneration={} canonicalWorkspacesRetired={} retiredStateRoots={} retiredBytes={}",
        args.day,
        committed_catalog_generation,
        canonical_retired,
        retired_state_root_count,
        catalog_plan
            .retired_bytes
            .saturating_add(retired_state_bytes),
    );
    if receipt_json {
        let receipt = serde_json::to_string(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.state-home-cleanup-receipt",
            "schemaVersion": 1,
            "state": "applied",
            "retainedForDays": args.day,
            "catalogGeneration": committed_catalog_generation,
            "canonicalWorkspacesRetired": canonical_retired,
            "retiredStateRoots": retired_state_root_count,
            "catalogPlan": catalog_plan,
        }))
        .map_err(|error| format!("failed to serialize workspace cleanup receipt: {error}"))?;
        eprintln!("{receipt}");
    }
    Ok(())
}

fn retired_state_root_selected(args: &CacheCleanArgs, object: &RetiredStateRoot) -> bool {
    if args.workspace_digest.is_some() {
        return false;
    }
    if let Some(object_id) = &args.object_id {
        return object_id == &object.object_id;
    }
    if let Some(workspace_root) = &args.workspace_root {
        let canonical = workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.clone());
        return canonical == object.checkout_root;
    }
    true
}

fn now_ms() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock precedes Unix epoch: {error}"))?
        .as_millis()
        .try_into()
        .map_err(|_| "system timestamp exceeds u64".to_string())
}

fn rollback_staged_workspaces(mut staged: Vec<StagedWorkspaceRetirement>) -> Result<(), String> {
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

fn reap_staged_workspaces(staged: Vec<StagedWorkspaceRetirement>) -> Vec<String> {
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
