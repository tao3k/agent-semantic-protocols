//! Thin Protocol adapter for Runtime Server lifecycle requests.

use std::path::{Path, PathBuf};

pub(crate) use agent_semantic_client_db::RuntimeServerSpawnReceipt;

fn server_dir(state_home: &Path) -> PathBuf {
    state_home.join("runtime/server")
}

pub(crate) async fn ensure_runtime_server(
    state_home: &Path,
    explicit: bool,
) -> Result<Option<RuntimeServerSpawnReceipt>, String> {
    let request = supervisor_request(state_home).await?;
    ensure_runtime_server_with_request(request, explicit).await
}

pub(crate) async fn ensure_runtime_server_after_identity_handoff(
    state_home: &Path,
) -> Result<Option<RuntimeServerSpawnReceipt>, String> {
    let request = supervisor_request_for_active_artifact(state_home).await?;
    ensure_runtime_server_with_request(request, false).await
}

async fn ensure_runtime_server_with_request(
    request: agent_semantic_client_db::runtime_server_supervisor::SupervisorRequest,
    explicit: bool,
) -> Result<Option<RuntimeServerSpawnReceipt>, String> {
    let state_home = request.state_home.clone();
    let outcome = agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
        .ensure_runtime_server(request, explicit)
        .await?;
    match outcome {
        agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::AlreadyResident => {
            Ok(None)
        }
        agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::SpawnAccepted => {
            read_runtime_server_spawn_receipt(&state_home)
                .await?
                .map(Some)
                .ok_or_else(|| "Runtime Server spawn completed without an owner receipt".to_owned())
        }
        unexpected => Err(format!(
            "Runtime Server supervisor returned an invalid ensure outcome: {unexpected:?}"
        )),
    }
}

pub(crate) async fn read_runtime_server_spawn_receipt(
    state_home: &Path,
) -> Result<Option<RuntimeServerSpawnReceipt>, String> {
    agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(state_home).await
}

pub(crate) async fn request_runtime_server_drain(state_home: &Path) -> Result<(), String> {
    let endpoint =
        agent_semantic_client_db::runtime_server_control::read_runtime_server_supervisor_endpoint(
            state_home,
        )
        .await?
        .ok_or_else(|| "Runtime Server endpoint is unavailable for drain".to_owned())?;
    agent_semantic_client_db::runtime_server_supervisor::request_runtime_server_drain(&endpoint)
        .await
}

pub(crate) async fn ensure_healthy_runtime_server(
    state_home: &Path,
) -> Result<agent_semantic_client_db::runtime_server_control::RuntimeServerControlReceipt, String> {
    ensure_runtime_server(state_home, false).await?;
    super::runtime_server::await_healthy_runtime_server_after_spawn().await?;
    super::runtime_server::observe_runtime_server_readiness(state_home).await
}

pub(crate) async fn ensure_healthy_runtime_server_after_identity_handoff(
    state_home: &Path,
) -> Result<agent_semantic_client_db::runtime_server_control::RuntimeServerControlReceipt, String> {
    ensure_runtime_server_after_identity_handoff(state_home).await?;
    super::runtime_server::await_healthy_runtime_server_after_spawn().await?;
    super::runtime_server::observe_runtime_server_readiness(state_home).await
}

async fn supervisor_request(
    state_home: &Path,
) -> Result<agent_semantic_client_db::runtime_server_supervisor::SupervisorRequest, String> {
    let current_exe =
        std::env::current_exe().map_err(|error| format!("resolve ASP invoker: {error}"))?;
    let receipt =
        agent_semantic_runtime::runtime_artifact_identity::read_runtime_artifact_identity(
            state_home, "asp",
        )
        .await?;
    agent_semantic_runtime::runtime_artifact_identity::admit_runtime_invoker(
        &current_exe,
        &receipt,
        &state_home.join("runtime/bin/asp"),
    )?;
    supervisor_request_for_active_artifact_with_receipt(state_home, &receipt).await
}

async fn supervisor_request_for_active_artifact(
    state_home: &Path,
) -> Result<agent_semantic_client_db::runtime_server_supervisor::SupervisorRequest, String> {
    let receipt =
        agent_semantic_runtime::runtime_artifact_identity::read_runtime_artifact_identity(
            state_home, "asp",
        )
        .await?;
    supervisor_request_for_active_artifact_with_receipt(state_home, &receipt).await
}

async fn supervisor_request_for_active_artifact_with_receipt(
    state_home: &Path,
    receipt: &agent_semantic_runtime::runtime_artifact_identity::RuntimeArtifactIdentityReceipt,
) -> Result<agent_semantic_client_db::runtime_server_supervisor::SupervisorRequest, String> {
    let runtime_artifact = canonical_runtime_artifact(state_home).await?;
    agent_semantic_runtime::runtime_artifact_identity::admit_runtime_invoker(
        &runtime_artifact,
        receipt,
        &state_home.join("runtime/bin/asp"),
    )?;
    Ok(
        agent_semantic_client_db::runtime_server_supervisor::SupervisorRequest {
            state_home: state_home.to_owned(),
            expected_executable: runtime_artifact.clone(),
            launch: agent_semantic_runtime::runtime_process_lifecycle::RuntimeProcessLaunchSpec {
                program: runtime_artifact,
                args: vec!["server".to_owned(), "daemon".to_owned()],
                current_dir: None,
                environment: vec![(
                    "ASP_STATE_HOME".to_owned(),
                    state_home.to_string_lossy().into_owned(),
                )],
                stderr: server_dir(state_home).join("owner-stderr.log"),
            },
        },
    )
}

async fn canonical_runtime_artifact(state_home: &Path) -> Result<PathBuf, String> {
    let stable_entry = state_home.join("runtime/bin/asp");
    let resolved = agent_semantic_runtime::runtime_process_lifecycle::canonicalize(&stable_entry)
        .await
        .map_err(|error| {
            format!(
                "canonical ASP Runtime Server binary is unavailable at {}: {error}",
                stable_entry.display()
            )
        })?;
    if !resolved.is_file() {
        return Err(format!(
            "canonical ASP Runtime Server binary target is not a file: {}",
            resolved.display()
        ));
    }
    // The stable entry is an invocation pointer, not process identity.  Owners
    // must be launched and recorded by their immutable digest-addressed path so
    // a subsequent active publication cannot rewrite the identity of a live
    // process and make verified drain impossible.
    Ok(resolved)
}
