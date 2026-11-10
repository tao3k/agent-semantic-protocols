// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime-owned reconciliation of the Hook's bounded local event ingress.

use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission;
use agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog;
use agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const RECONCILE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(10);

pub(super) fn spawn_reconciler(
    state_home: &Path,
    workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
    admission_catalog: RuntimeWorkspaceAdmissionCatalog,
    generation_admission: Arc<WorkspaceGenerationAdmission>,
    task_scope: &agent_semantic_workspace_scheduler::RuntimeServerTaskScope,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<agent_semantic_workspace_scheduler::RuntimeServerOwnedTask<Result<(), String>>, String>
{
    let serving = agent_semantic_artifacts::StateHomeLayout::new(state_home)
        .runtime_state()
        .serving();
    task_scope.spawn(
        "runtime-hook-memory-inbox",
        reconcile(
            serving.hook_memory_inbox(),
            serving.hook_memory_inbox_acknowledgement(),
            workspace_registry,
            admission_catalog,
            generation_admission,
            task_scope.clone(),
            shutdown,
        ),
    )
}

pub(super) async fn reconcile(
    inbox_path: PathBuf,
    acknowledgement_path: PathBuf,
    workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
    admission_catalog: RuntimeWorkspaceAdmissionCatalog,
    generation_admission: Arc<WorkspaceGenerationAdmission>,
    task_scope: agent_semantic_workspace_scheduler::RuntimeServerTaskScope,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let mut acknowledged = read_acknowledgement(&acknowledgement_path).await?;
    let reader_path = inbox_path.clone();
    let reader = Arc::new(
        task_scope
            .spawn_blocking("runtime-hook-inbox-open", move || {
                agent_semantic_hook::hook_memory_inbox::HookMemoryInboxReader::open_or_create(
                    &reader_path,
                )
            })?
            .join()
            .await
            .map_err(|error| format!("Hook inbox resident reader task failed: {error}"))?
            .map_err(|failure| {
                format!(
                    "Hook inbox resident reader failed: reasonKind={:?} detail={}",
                    failure.reason_kind, failure.detail
                )
            })?,
    );
    let mut compaction_pending = acknowledged > 0;
    let mut interval = tokio::time::interval(RECONCILE_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return Ok(());
                }
            }
            _ = interval.tick() => {}
        }
        let latest = match reader.latest_published_sequence() {
            Ok(sequence) => sequence,
            Err(failure)
                if failure.reason_kind
                    == agent_semantic_client_protocol::HookMemoryInboxFailureReason::LockBudgetExceeded =>
            {
                continue;
            }
            Err(failure) => {
                return Err(format!(
                    "Hook inbox boundary read failed: reasonKind={:?} detail={}",
                    failure.reason_kind, failure.detail
                ));
            }
        };
        let events = if latest > acknowledged {
            let resident_reader = Arc::clone(&reader);
            task_scope
                .spawn_blocking("runtime-hook-inbox-read", move || {
                    resident_reader.read_after(acknowledged)
                })?
                .join()
                .await
                .map_err(|error| format!("Hook inbox reader task failed: {error}"))?
                .map_err(|failure| {
                    format!(
                        "Hook inbox read failed: reasonKind={:?} detail={}",
                        failure.reason_kind, failure.detail
                    )
                })?
        } else {
            Vec::new()
        };
        for envelope in events {
            let sequence = envelope.inbox_sequence;
            let Some(event) = envelope.workspace_mutation_event()? else {
                // Other entry kinds need their own materializer before they can
                // be acknowledged. Never skip an unknown predecessor.
                break;
            };
            if let Err(error) = reconcile_workspace_mutation(
                event,
                &workspace_registry,
                &admission_catalog,
                &generation_admission,
            )
            .await
            {
                eprintln!(
                    "[runtime-hook-memory-inbox] state=pending inboxSequence={sequence} error={error}"
                );
                break;
            }
            publish_acknowledgement(&acknowledgement_path, sequence).await?;
            acknowledged = sequence;
            compaction_pending = true;
        }
        if compaction_pending {
            let compact_path = inbox_path.clone();
            let compaction = task_scope
                .spawn_blocking("runtime-hook-inbox-compact", move || {
                    agent_semantic_hook::hook_memory_inbox::compact_through(
                        &compact_path,
                        acknowledged,
                    )
                })?
                .join()
                .await
                .map_err(|error| format!("Hook inbox compactor task failed: {error}"))?;
            match compaction {
                Ok(_) => compaction_pending = false,
                Err(failure) => eprintln!(
                    "[runtime-hook-memory-inbox] state=compaction-pending reasonKind={:?} detail={}",
                    failure.reason_kind, failure.detail
                ),
            }
        }
    }
}

async fn reconcile_workspace_mutation(
    event: agent_semantic_client_protocol::HookWorkspaceMutationEvent,
    workspace_registry: &RuntimeServerWorkspaceRegistry,
    admission_catalog: &RuntimeWorkspaceAdmissionCatalog,
    generation_admission: &WorkspaceGenerationAdmission,
) -> Result<(), String> {
    let project_root = PathBuf::from(&event.project_root);
    let binding = admission_catalog
        .snapshot()
        .iter()
        .find(|entry| entry.project_root == project_root)
        .cloned()
        .ok_or_else(|| {
            format!(
                "Hook inbox workspace is not admitted: projectRoot={}",
                project_root.display()
            )
        })?;
    let changed_paths = event
        .changed_paths
        .iter()
        .map(|path| project_root.join(path))
        .collect::<Vec<_>>();

    // Byte-search truth becomes resident first. Parser/topology repair
    // follows in the same mutation transaction but never blocks Search.
    workspace_registry
        .publish_owner_content_mutation_from_paths(
            event.mutation_id.clone(),
            &binding.workspace_identity,
            &project_root,
            &changed_paths,
        )
        .await?;
    let candidate = agent_semantic_client_db::runtime_server_admission::discover_workspace_generation_candidate(
                &project_root,
            )
            .await?;
    generation_admission
        .admit_observed_mutation_terminal(
            event.mutation_id,
            binding.workspace_identity,
            project_root,
            changed_paths,
            candidate,
        )
        .await?;
    Ok(())
}

async fn read_acknowledgement(path: &Path) -> Result<u64, String> {
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            let text = std::str::from_utf8(&bytes)
                .map_err(|error| format!("decode Hook inbox acknowledgement UTF-8: {error}"))?;
            text.trim()
                .parse::<u64>()
                .map_err(|error| format!("decode Hook inbox acknowledgement sequence: {error}"))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(format!(
            "read Hook inbox acknowledgement {}: {error}",
            path.display()
        )),
    }
}

async fn publish_acknowledgement(path: &Path, sequence: u64) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Hook inbox acknowledgement path has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| format!("create Hook inbox acknowledgement directory: {error}"))?;
    let temporary = path.with_extension(format!("ack.tmp-{}", std::process::id()));
    tokio::fs::write(&temporary, format!("{sequence}\n"))
        .await
        .map_err(|error| format!("write staged Hook inbox acknowledgement: {error}"))?;
    tokio::fs::rename(&temporary, path)
        .await
        .map_err(|error| format!("publish Hook inbox acknowledgement: {error}"))
}
