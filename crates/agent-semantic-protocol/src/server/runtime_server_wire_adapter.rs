//! Thin Protocol adapter for Runtime Server lifecycle requests.

use std::path::Path;

pub(crate) use agent_semantic_client_db::RuntimeServerSpawnReceipt;

pub(crate) async fn read_runtime_server_spawn_receipt(
    state_home: &Path,
) -> Result<Option<RuntimeServerSpawnReceipt>, String> {
    agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(state_home).await
}

pub(crate) enum RuntimeServerActivationReconciliation {
    Supervisor(agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome),
    Healthy(agent_semantic_client_db::runtime_server_control::RuntimeServerControlReceipt),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeServerActivationAuthority {
    OperatorStart,
    ClientBootstrap,
}

impl RuntimeServerActivationAuthority {
    pub(crate) const fn is_explicit_operator_start(self) -> bool {
        matches!(self, Self::OperatorStart)
    }
}

pub(crate) fn validate_activation_ready_binding(
    receipt: &agent_semantic_client_db::RuntimeServerActivationReadyReceipt,
    event: &agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactActivationEvent,
    spawn: &agent_semantic_client_db::RuntimeServerSpawnReceipt,
) -> Result<(), String> {
    let spawn_receipt_digest =
        agent_semantic_client_db::runtime_server_lifecycle::spawn_receipt_digest(spawn)?;
    if receipt.schema_id != "agent.semantic-protocols.runtime-activation-ready-receipt"
        || receipt.schema_version != "1"
        || receipt.state != "ready"
        || receipt.activation_generation != event.activation_generation
        || receipt.artifact_digest != event.artifact_digest
        || receipt.owner_epoch == 0
        || receipt.launcher_receipt_digest != spawn_receipt_digest
    {
        return Err(
            "state=runtime-activation-ready-failed reasonKind=ready-authority-binding-mismatch"
                .to_owned(),
        );
    }
    Ok(())
}

pub(crate) async fn ensure_healthy_runtime_server_for_activation_event(
    state_home: &Path,
    event: &agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactActivationEvent,
    serving_digest: Option<&agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest>,
) -> Result<agent_semantic_client_db::runtime_server_control::RuntimeServerControlReceipt, String> {
    match reconcile_runtime_server_activation_event(
        state_home,
        event,
        serving_digest,
        true,
        RuntimeServerActivationAuthority::OperatorStart,
    )
    .await?
    {
        RuntimeServerActivationReconciliation::Healthy(receipt) => Ok(receipt),
        RuntimeServerActivationReconciliation::Supervisor(_) => {
            Err("Runtime activation reconciliation returned before healthy publication".to_owned())
        }
    }
}

pub(crate) async fn ensure_runtime_server_for_activation_event(
    state_home: &Path,
    event: &agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactActivationEvent,
    serving_digest: Option<&agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest>,
) -> Result<agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome, String> {
    match reconcile_runtime_server_activation_event(
        state_home,
        event,
        serving_digest,
        false,
        RuntimeServerActivationAuthority::ClientBootstrap,
    )
    .await?
    {
        RuntimeServerActivationReconciliation::Supervisor(supervisor) => Ok(supervisor),
        RuntimeServerActivationReconciliation::Healthy(_) => Err(
            "Runtime bootstrap reconciliation crossed the healthy publication boundary".to_owned(),
        ),
    }
}

pub(crate) async fn reconcile_runtime_server_activation_event(
    state_home: &Path,
    event: &agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactActivationEvent,
    serving_digest: Option<&agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest>,
    wait_for_healthy: bool,
    authority: RuntimeServerActivationAuthority,
) -> Result<RuntimeServerActivationReconciliation, String> {
    if event.artifact_digest != event.candidate_identity.artifact_digest
        || event.artifact_path != event.candidate_identity.artifact_path
        || event.publication_nonce != event.candidate_identity.publication_nonce
    {
        return Err(
            "state=runtime-server-activation-failed reasonKind=candidate-identity-binding-mismatch"
                .to_owned(),
        );
    }
    let bytes = tokio::fs::read(&event.artifact_path)
        .await
        .map_err(|error| format!("read event-bound Runtime candidate: {error}"))?;
    let observed =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(&bytes);
    if observed != event.artifact_digest {
        return Err(format!(
            "state=runtime-server-activation-failed reasonKind=candidate-content-digest-mismatch expected={} observed={}",
            event.artifact_digest, observed
        ));
    }

    let mut ready_listener = if wait_for_healthy {
        Some(
            agent_semantic_client_db::runtime_server_lifecycle::bind_activation_ready_listener(
                state_home,
                event.activation_generation,
                &event.publication_nonce,
            )
            .await?,
        )
    } else {
        None
    };
    let mut environment = vec![
        (
            "ASP_STATE_HOME".to_owned(),
            state_home.to_string_lossy().into_owned(),
        ),
        (
            "ASP_RUNTIME_BINARY_CONTENT_DIGEST".to_owned(),
            event.artifact_digest.to_string(),
        ),
    ];
    if let Some(listener) = ready_listener.as_ref() {
        environment.push((
            "ASP_RUNTIME_ACTIVATION_READY_SOCKET".to_owned(),
            listener.path().to_string_lossy().into_owned(),
        ));
    }
    let owner_stderr_path = state_home.join("runtime/server/owner-stderr.log");
    let request =
        agent_semantic_client_db::runtime_server_supervisor::SupervisorRequest::for_activation(
            state_home.to_owned(),
            event.artifact_path.clone(),
            event.activation_generation,
            event.artifact_digest.clone(),
            event.previous_artifact_digest.clone(),
            event.artifact_path.clone(),
            vec!["server".to_owned(), "daemon".to_owned()],
            None,
            environment,
            owner_stderr_path.clone(),
        );
    if !wait_for_healthy {
        let supervisor =
            agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
                .ensure_runtime_server(request, authority.is_explicit_operator_start())
                .await?;
        return Ok(RuntimeServerActivationReconciliation::Supervisor(
            supervisor,
        ));
    }
    let mut supervision =
        agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
            .ensure_runtime_server_monitored(request, authority.is_explicit_operator_start())
            .await?;
    if supervision.outcome
        == agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::SpawnAccepted
    {
        let mut process = supervision.process.take().ok_or_else(|| {
            "Runtime activation SpawnAccepted outcome requires a monitored child".to_owned()
        })?;
        let listener = ready_listener
            .as_mut()
            .ok_or_else(|| "Runtime activation wait requires a bound ready listener".to_owned())?;
        let receipt = tokio::select! {
            receipt = listener.receive() => receipt?,
            exit = process.wait() => {
                let exit = exit?;
                let daemon_stderr = tokio::fs::read_to_string(&owner_stderr_path)
                    .await
                    .unwrap_or_else(|error| format!("unavailable: {error}"));
                return Err(serde_json::json!({
                    "schemaId": "agent.semantic-protocols.runtime-activation-ready-receipt",
                    "schemaVersion": "1",
                    "state": "failed",
                    "reasonKind": "runtime-owner-exited-before-ready",
                    "exitStatus": exit.to_string(),
                    "daemonStderr": daemon_stderr,
                    "activationGeneration": event.activation_generation,
                    "artifactDigest": event.artifact_digest,
                }).to_string());
            }
        };
        let spawn =
            agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(state_home)
                .await?
                .ok_or_else(|| {
                    "Runtime activation ready receipt has no owner-spawn authority".to_owned()
                })?;
        validate_activation_ready_binding(&receipt, event, &spawn)?;
        if let Some(exit) = process.try_wait()? {
            return Err(serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-activation-ready-receipt",
                "schemaVersion": "1",
                "state": "failed",
                "reasonKind": "runtime-owner-exited-after-ready",
                "exitStatus": exit.to_string(),
                "activationGeneration": event.activation_generation,
                "artifactDigest": event.artifact_digest,
            })
            .to_string());
        }
    }
    let ready = crate::server::runtime_server::observe_runtime_server_readiness(state_home).await?;
    let transaction =
        agent_semantic_client_db::runtime_server_lifecycle::observe_resident_transaction(
            state_home,
        )
        .await?;
    eprintln!(
        "[runtime-server-resident-transaction] {}",
        serde_json::to_string(&transaction)
            .map_err(|error| format!("encode Runtime resident transaction receipt: {error}"))?
    );
    if transaction.activation_generation != event.activation_generation
        || transaction.applied_artifact_digest != event.artifact_digest
    {
        return Err(
            "state=runtime-activation-ready-failed reasonKind=transaction-authority-mismatch"
                .to_owned(),
        );
    }
    let _ = serving_digest;
    Ok(RuntimeServerActivationReconciliation::Healthy(ready))
}

#[cfg(test)]
#[path = "../../tests/unit/server/runtime_server_wire_adapter.rs"]
mod tests;
