use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use crate::runtime_server_admission::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionState,
    discover_workspace_generation_candidate,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct GenerationRepairKey {
    workspace_identity: String,
    project_root: PathBuf,
}

fn active_repairs() -> &'static Mutex<HashSet<GenerationRepairKey>> {
    static ACTIVE_REPAIRS: OnceLock<Mutex<HashSet<GenerationRepairKey>>> = OnceLock::new();
    ACTIVE_REPAIRS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn repair_runtime() -> Result<&'static tokio::runtime::Runtime, String> {
    static RUNTIME: std::sync::OnceLock<Result<tokio::runtime::Runtime, String>> =
        std::sync::OnceLock::new();
    match RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("asp-generation-repair")
            .enable_all()
            .build()
            .map_err(|error| format!("create Runtime Server generation repair runtime: {error}"))
    }) {
        Ok(runtime) => Ok(runtime),
        Err(error) => Err(error.clone()),
    }
}

/// Queue cold generation repair under Runtime Server ownership.
///
/// Returning from the caller, including an agent-facing wall timeout, cannot
/// cancel this task because it owns both the admission handle and identities.
pub(super) fn enqueue(
    admission: Arc<WorkspaceGenerationAdmission>,
    workspace_identity: String,
    project_root: PathBuf,
) -> Result<bool, String> {
    let runtime = repair_runtime()?;
    let key = GenerationRepairKey {
        workspace_identity,
        project_root,
    };
    {
        let mut active = active_repairs()
            .lock()
            .map_err(|_| "workspace generation repair coordinator is poisoned".to_owned())?;
        if !active.insert(key.clone()) {
            return Ok(false);
        }
    }

    runtime.spawn(async move {
        let repair = async {
            let candidate = discover_workspace_generation_candidate(&key.project_root).await?;
            let admitted = admission
                .admit(
                    key.workspace_identity.clone(),
                    key.project_root.clone(),
                    candidate,
                )
                .await?;
            if admitted.state == WorkspaceGenerationAdmissionState::Building {
                admission
                    .wait_terminal(&key.workspace_identity, &key.project_root)
                    .await?;
            }
            Ok::<(), String>(())
        }
        .await;

        if let Err(error) = repair {
            eprintln!(
                "Runtime Server background generation repair failed: workspaceIdentity={} projectRoot={} error={error}",
                key.workspace_identity,
                key.project_root.display()
            );
        }
        if let Ok(mut active) = active_repairs().lock() {
            active.remove(&key);
        }
    });
    Ok(true)
}

#[cfg(test)]
#[path = "../tests/unit/workspace_db_ipc_server_generation_repair.rs"]
mod tests;
