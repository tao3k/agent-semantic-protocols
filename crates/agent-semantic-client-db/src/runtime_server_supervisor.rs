//! Protocol-neutral Runtime Server supervisor boundary.

use std::path::PathBuf;

use serde::Serialize;

pub struct SupervisorRequest {
    pub state_home: PathBuf,
    pub expected_executable: PathBuf,
    pub launch: agent_semantic_runtime::runtime_process_lifecycle::RuntimeProcessLaunchSpec,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SupervisorOutcome {
    AlreadyResident,
    SpawnAccepted,
    OwnerStale,
    Failed,
}

pub struct RuntimeServerSupervisor;

async fn acquire_supervisor_transaction(
    state_home: &std::path::Path,
) -> Result<crate::runtime_server_control::RuntimeServerSupervisorTransaction, String> {
    crate::runtime_server_control::acquire_runtime_server_supervisor_transaction(state_home).await
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerStopOutcome {
    pub endpoint_path: String,
    pub endpoint_removed: bool,
    pub control_socket_removed: bool,
    pub data_plane_socket_removed: bool,
    pub status_memory_removed: bool,
    pub operator_stop_recorded: bool,
}

pub async fn retire_runtime_server_owner_for_handoff(
    state_home: &std::path::Path,
    expected_endpoint: &crate::runtime_server_control::RuntimeServerEndpoint,
) -> Result<(), String> {
    let current_endpoint =
        crate::runtime_server_control::read_runtime_server_supervisor_endpoint(state_home)
            .await?
            .ok_or_else(|| {
                "cannot retire Runtime Server owner without the current endpoint".to_owned()
            })?;
    let owner = crate::runtime_server_lifecycle::read_owner_receipt(state_home)
        .await?
        .ok_or_else(|| {
            "cannot retire Runtime Server owner without the current owner receipt".to_owned()
        })?;
    validate_runtime_server_owner_binding(expected_endpoint, &current_endpoint, &owner)?;
    crate::runtime_server_lifecycle::remove_owner_receipt(state_home).await
}

pub fn validate_runtime_server_owner_binding(
    endpoint: &crate::runtime_server_control::RuntimeServerEndpoint,
    current_endpoint: &crate::runtime_server_control::RuntimeServerEndpoint,
    owner: &crate::RuntimeServerSpawnReceipt,
) -> Result<(), String> {
    if endpoint.owner_process_id == 0 {
        return Err("Runtime Server endpoint owner identity is unbound".to_owned());
    }
    if current_endpoint.owner_epoch != endpoint.owner_epoch
        || current_endpoint.owner_process_id != endpoint.owner_process_id
    {
        return Err("Runtime Server owner changed during verified termination".to_owned());
    }
    if owner.process_id != endpoint.owner_process_id
        || owner.runtime_artifact_path != endpoint.runtime_artifact_path
    {
        return Err("Runtime Server endpoint and owner receipt identities differ".to_owned());
    }
    Ok(())
}

async fn terminate_endpoint_owner(
    state_home: &std::path::Path,
    endpoint: &crate::runtime_server_control::RuntimeServerEndpoint,
    force: bool,
) -> Result<crate::runtime_server_lifecycle_coordinator::OwnerClassification, String> {
    let classification = classify_endpoint_owner(state_home, endpoint).await?;
    if classification == crate::runtime_server_lifecycle_coordinator::OwnerClassification::Live {
        let coordinator =
            crate::runtime_server_lifecycle_coordinator::RuntimeServerLifecycleCoordinator::new(
                state_home,
                std::path::Path::new(&endpoint.runtime_artifact_path),
            );
        coordinator
            .terminate_verified(endpoint.owner_process_id, force)
            .await?;
    }
    Ok(classification)
}

async fn classify_endpoint_owner(
    state_home: &std::path::Path,
    endpoint: &crate::runtime_server_control::RuntimeServerEndpoint,
) -> Result<crate::runtime_server_lifecycle_coordinator::OwnerClassification, String> {
    let owner = crate::runtime_server_lifecycle::read_owner_receipt(state_home)
        .await?
        .ok_or_else(|| "Runtime Server owner receipt is missing".to_owned())?;
    let current_endpoint =
        crate::runtime_server_control::read_runtime_server_supervisor_endpoint(state_home)
            .await?
            .ok_or_else(|| "Runtime Server endpoint disappeared".to_owned())?;
    validate_runtime_server_owner_binding(endpoint, &current_endpoint, &owner)?;
    let coordinator =
        crate::runtime_server_lifecycle_coordinator::RuntimeServerLifecycleCoordinator::new(
            state_home,
            std::path::Path::new(&endpoint.runtime_artifact_path),
        );
    let classification = coordinator
        .classify(Some(endpoint.owner_process_id))
        .await?;
    Ok(classification)
}

fn runtime_server_status_requires_drain(
    state: &crate::runtime_server_control::RuntimeServerState,
) -> bool {
    state != &crate::runtime_server_control::RuntimeServerState::Draining
}

#[cfg(test)]
#[path = "../tests/unit/runtime_server_supervisor_lifecycle_transition.rs"]
mod lifecycle_transition_tests;

async fn retire_undecodable_endpoint_owner(request: &SupervisorRequest) -> Result<bool, String> {
    let Some(owner) =
        crate::runtime_server_lifecycle::read_owner_receipt(&request.state_home).await?
    else {
        // With no independently verified live owner, the supervisor may remove
        // only the canonical invalid endpoint path.  It must not parse or trust
        // attacker-controlled endpoint contents to discover other paths.
        return Ok(false);
    };
    let Some(binding) = crate::runtime_server_control::read_runtime_server_endpoint_owner_binding(
        &request.state_home,
    )
    .await?
    else {
        return Ok(false);
    };
    if owner.process_id != binding.owner_process_id
        || owner.runtime_artifact_path != binding.runtime_artifact_path
        || owner.state_home != request.state_home.display().to_string()
    {
        return Err(
            "undecodable Runtime Server endpoint and owner receipt identities differ; refusing unbound termination"
                .to_owned(),
        );
    }
    let coordinator =
        crate::runtime_server_lifecycle_coordinator::RuntimeServerLifecycleCoordinator::new(
            &request.state_home,
            std::path::Path::new(&binding.runtime_artifact_path),
        );
    match coordinator.classify(Some(binding.owner_process_id)).await? {
        crate::runtime_server_lifecycle_coordinator::OwnerClassification::Live => {
            coordinator
                .terminate_verified(binding.owner_process_id, false)
                .await?;
            let exit = crate::runtime_server_lifecycle::await_owner_exit(
                &request.state_home,
                binding.owner_epoch,
            )
            .await?;
            if !exit.clean_drain {
                return Err(format!(
                    "undecodable Runtime Server owner {} exited without a clean drain: errors={:?}",
                    binding.owner_epoch, exit.errors
                ));
            }
        }
        crate::runtime_server_lifecycle_coordinator::OwnerClassification::Stale => {}
        crate::runtime_server_lifecycle_coordinator::OwnerClassification::Missing => {
            return Err("undecodable Runtime Server endpoint owner identity is missing".to_owned());
        }
    }
    Ok(true)
}

impl RuntimeServerSupervisor {
    pub async fn ensure_runtime_server(
        &self,
        request: SupervisorRequest,
        explicit: bool,
    ) -> Result<SupervisorOutcome, String> {
        if explicit {
            crate::runtime_server_lifecycle::clear_operator_stopped(&request.state_home).await?;
        } else if crate::runtime_server_lifecycle::operator_stopped(&request.state_home).await? {
            return Err(serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-server-operator-stop",
                "schemaVersion": "1",
                "state": "stopped",
                "reasonKind": "operator-stop-is-authoritative",
                "nextCommand": "asp server start",
            })
            .to_string());
        }
        crate::runtime_server_lifecycle::create_run_intent(&request.state_home).await?;
        crate::runtime_server_lifecycle::remove_stale(&request.state_home).await?;
        // Handoff is one serialized transaction: observe, retire the exact old
        // owner, clean its receipt, and publish the next owner receipt. Without
        // this reservation, a concurrent ensure could publish a replacement
        // between invalid-endpoint classification and cleanup.
        let transaction = acquire_supervisor_transaction(&request.state_home).await?;
        match crate::runtime_server_control::read_runtime_server_supervisor_endpoint(
            &request.state_home,
        )
        .await
        {
            Ok(Some(endpoint)) => {
                let desired_identity =
                    agent_semantic_runtime::runtime_artifact_identity::read_runtime_artifact_identity(
                        &request.state_home,
                        "asp",
                    )
                    .await?
                    .identity();
                let status = crate::runtime_server_control::call_runtime_server(
                    &endpoint,
                    crate::runtime_server_control::RuntimeServerOperation::Status,
                    endpoint.runtime_binary_identity.clone(),
                    lifecycle_request_id("status"),
                )
                .await;
                if status.as_ref().is_ok_and(|receipt| {
                    receipt.state == crate::runtime_server_control::RuntimeServerState::Healthy
                        && endpoint.runtime_binary_identity == desired_identity
                }) {
                    return Ok(SupervisorOutcome::AlreadyResident);
                }
                let owner_was_live = match status.as_ref() {
                    Ok(receipt) if runtime_server_status_requires_drain(&receipt.state) => {
                        request_runtime_server_drain(&endpoint).await?;
                        true
                    }
                    Ok(_) => {
                        // A Draining status is a durable mmap publication: the
                        // drain already linearized even when the control socket
                        // has since closed. Never send a second drain request.
                        classify_endpoint_owner(&request.state_home, &endpoint).await?
                            == crate::runtime_server_lifecycle_coordinator::OwnerClassification::Live
                    }
                    Err(_) => terminate_endpoint_owner(&request.state_home, &endpoint, false)
                        .await?
                        == crate::runtime_server_lifecycle_coordinator::OwnerClassification::Live,
                };
                let exit = if owner_was_live {
                    Some(
                        crate::runtime_server_lifecycle::await_owner_exit(
                            &request.state_home,
                            endpoint.owner_epoch,
                        )
                        .await?,
                    )
                } else {
                    crate::runtime_server_lifecycle::read_owner_exit_for(
                        &request.state_home,
                        endpoint.owner_epoch,
                    )
                    .await?
                };
                if let Some(exit) = exit {
                    if !exit.clean_drain {
                        return Err(format!(
                            "Runtime Server generation drain failed: errors={:?}",
                            exit.errors
                        ));
                    }
                }
                crate::runtime_server_control::cleanup_runtime_server_endpoint(
                    &request.state_home,
                    &endpoint,
                )
                .await?;
                crate::runtime_server_lifecycle::remove_owner_receipt(&request.state_home).await?;
            }
            Ok(None) => {}
            Err(endpoint_error) => {
                retire_undecodable_endpoint_owner(&request)
                    .await
                    .map_err(|error| format!("{error}: {endpoint_error}"))?;
                crate::runtime_server_control::cleanup_invalid_runtime_server_endpoint(
                    &request.state_home,
                )
                .await
                .map_err(|error| {
                    format!(
                        "remove invalid Runtime Server endpoint after {endpoint_error}: {error}"
                    )
                })?;
                crate::runtime_server_lifecycle::remove_owner_receipt(&request.state_home).await?;
            }
        }
        self.ensure_owner_in_transaction(request, transaction).await
    }

    pub async fn classify_owner(
        &self,
        request: &SupervisorRequest,
    ) -> Result<SupervisorOutcome, String> {
        let receipt =
            crate::runtime_server_lifecycle::read_owner_receipt(&request.state_home).await?;
        let coordinator =
            crate::runtime_server_lifecycle_coordinator::RuntimeServerLifecycleCoordinator::new(
                &request.state_home,
                &request.expected_executable,
            );
        Ok(match receipt {
            Some(receipt)
                if coordinator.classify(Some(receipt.process_id)).await?
                    == crate::runtime_server_lifecycle_coordinator::OwnerClassification::Live =>
            {
                SupervisorOutcome::AlreadyResident
            }
            Some(_) => SupervisorOutcome::OwnerStale,
            None => SupervisorOutcome::SpawnAccepted,
        })
    }

    pub async fn ensure_owner(
        &self,
        request: SupervisorRequest,
    ) -> Result<SupervisorOutcome, String> {
        let transaction = acquire_supervisor_transaction(&request.state_home).await?;
        self.ensure_owner_in_transaction(request, transaction).await
    }

    async fn ensure_owner_in_transaction(
        &self,
        request: SupervisorRequest,
        transaction: crate::runtime_server_control::RuntimeServerSupervisorTransaction,
    ) -> Result<SupervisorOutcome, String> {
        let reservation = match crate::runtime_server_control::try_acquire_runtime_server_election(
            &request.state_home,
        )
        .await?
        {
            crate::runtime_server_control::RuntimeServerElectionAttempt::Acquired(reservation) => {
                reservation
            }
            crate::runtime_server_control::RuntimeServerElectionAttempt::Contended => {
                crate::runtime_server_control::wait_for_runtime_server_election(&request.state_home)
                    .await?
            }
        };
        match self.classify_owner(&request).await? {
            SupervisorOutcome::AlreadyResident => return Ok(SupervisorOutcome::AlreadyResident),
            SupervisorOutcome::OwnerStale => {
                crate::runtime_server_lifecycle::remove_owner_receipt(&request.state_home).await?;
            }
            SupervisorOutcome::SpawnAccepted | SupervisorOutcome::Failed => {}
        }
        let launch =
            agent_semantic_runtime::runtime_process_lifecycle::launch_detached(request.launch)
                .await?;
        let receipt = crate::RuntimeServerSpawnReceipt {
            schema_id: "agent.semantic-protocols.runtime-server-owner-spawn.v1".into(),
            schema_version: "1".into(),
            process_id: launch.process_id,
            nonce: format!("owner-{}", launch.process_id),
            state_home: request.state_home.display().to_string(),
            runtime_artifact_path: request.expected_executable.display().to_string(),
        };
        crate::runtime_server_lifecycle::write_owner_receipt(&request.state_home, &receipt).await?;
        drop(reservation);
        drop(transaction);
        Ok(SupervisorOutcome::SpawnAccepted)
    }

    pub async fn stop_runtime_server(
        &self,
        state_home: &std::path::Path,
    ) -> Result<RuntimeServerStopOutcome, String> {
        crate::runtime_server_lifecycle::remove_stale(state_home).await?;
        crate::runtime_server_lifecycle::remove_run_intent(state_home).await?;
        let endpoint =
            crate::runtime_server_control::read_runtime_server_supervisor_endpoint(state_home)
                .await?;
        if let Some(endpoint) = endpoint.as_ref() {
            let graceful_drain = crate::runtime_server_control::call_runtime_server(
                endpoint,
                crate::runtime_server_control::RuntimeServerOperation::Restart,
                endpoint.runtime_binary_identity.clone(),
                lifecycle_request_id("control-stop"),
            )
            .await;
            let exit = if matches!(
                graceful_drain,
                Ok(ref receipt)
                    if receipt.state == crate::runtime_server_control::RuntimeServerState::Draining
            ) {
                Some(
                    crate::runtime_server_lifecycle::await_owner_exit(
                        state_home,
                        endpoint.owner_epoch,
                    )
                    .await?,
                )
            } else {
                match terminate_endpoint_owner(state_home, endpoint, false).await? {
                    crate::runtime_server_lifecycle_coordinator::OwnerClassification::Live => Some(
                        crate::runtime_server_lifecycle::await_owner_exit(
                            state_home,
                            endpoint.owner_epoch,
                        )
                        .await?,
                    ),
                    crate::runtime_server_lifecycle_coordinator::OwnerClassification::Stale
                    | crate::runtime_server_lifecycle_coordinator::OwnerClassification::Missing => {
                        None
                    }
                }
            };
            if exit.as_ref().is_some_and(|exit| !exit.clean_drain) {
                let exit = exit.expect("checked exit receipt");
                return Err(format!(
                    "Runtime Server owner {} exited after a failed service drain: errors={:?}",
                    exit.owner_epoch, exit.errors
                ));
            }
            crate::runtime_server_control::cleanup_runtime_server_endpoint(state_home, endpoint)
                .await?;
        }
        crate::runtime_server_lifecycle::remove_owner_receipt(state_home).await?;
        crate::runtime_server_lifecycle::mark_operator_stopped(state_home).await?;
        let endpoint_path =
            crate::runtime_server_control::runtime_server_endpoint_path(state_home)?;
        let endpoint_removed = !tokio::fs::try_exists(&endpoint_path)
            .await
            .map_err(|error| error.to_string())?;
        let (control_socket_removed, data_plane_socket_removed, status_memory_removed) =
            match endpoint {
                Some(endpoint) => (
                    !tokio::fs::try_exists(&endpoint.socket_path)
                        .await
                        .map_err(|error| error.to_string())?,
                    !tokio::fs::try_exists(&endpoint.data_plane_socket_path)
                        .await
                        .map_err(|error| error.to_string())?,
                    !tokio::fs::try_exists(&endpoint.status_memory_path)
                        .await
                        .map_err(|error| error.to_string())?,
                ),
                None => (true, true, true),
            };
        if !endpoint_removed
            || !control_socket_removed
            || !data_plane_socket_removed
            || !status_memory_removed
        {
            return Err("Runtime Server stopped but terminal artifacts remain".to_owned());
        }
        Ok(RuntimeServerStopOutcome {
            endpoint_path: endpoint_path.display().to_string(),
            endpoint_removed,
            control_socket_removed,
            data_plane_socket_removed,
            status_memory_removed,
            operator_stop_recorded: true,
        })
    }
}

pub async fn request_runtime_server_drain(
    endpoint: &crate::runtime_server_control::RuntimeServerEndpoint,
) -> Result<(), String> {
    let receipt = crate::runtime_server_control::call_runtime_server(
        endpoint,
        crate::runtime_server_control::RuntimeServerOperation::Restart,
        endpoint.runtime_binary_identity.clone(),
        lifecycle_request_id("generation-drain"),
    )
    .await?;
    if receipt.state != crate::runtime_server_control::RuntimeServerState::Draining {
        return Err("Runtime Server refused graceful drain".to_owned());
    }
    Ok(())
}

fn lifecycle_request_id(kind: &str) -> String {
    let seed = format!(
        "{kind}:{}:{}",
        agent_semantic_runtime::runtime_process_lifecycle::current_process_id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos()),
    );
    format!("blake3-256:{}", blake3::hash(seed.as_bytes()).to_hex())
}
