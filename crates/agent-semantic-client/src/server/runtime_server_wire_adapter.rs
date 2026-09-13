// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Thin Protocol adapter for Runtime Server lifecycle requests.

use std::path::Path;

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

fn runtime_activation_environment(
    state_home: &Path,
    event: &agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent,
) -> Vec<(String, String)> {
    vec![
        (
            "ASP_STATE_HOME".to_owned(),
            state_home.to_string_lossy().into_owned(),
        ),
        (
            "ASP_RUNTIME_BINARY_CONTENT_DIGEST".to_owned(),
            event.artifact_digest.to_string(),
        ),
    ]
}

fn resident_transaction_identity_matches(
    event: &agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent,
    publication_nonce: &str,
    applied_artifact_digest: &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
) -> bool {
    publication_nonce == event.publication_nonce
        && applied_artifact_digest == &event.artifact_digest
}

pub(crate) async fn ensure_healthy_runtime_server_for_activation_event(
    state_home: &Path,
    event: &agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent,
    serving_digest: Option<&agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest>,
) -> Result<agent_semantic_client_db::runtime_server_control::RuntimeServerControlReceipt, String> {
    reconcile_runtime_server_activation_event(
        state_home,
        event,
        serving_digest,
        RuntimeServerActivationAuthority::OperatorStart,
        false,
    )
    .await
}

pub(crate) async fn restart_healthy_runtime_server_for_activation_event(
    state_home: &Path,
    event: &agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent,
) -> Result<agent_semantic_client_db::runtime_server_control::RuntimeServerControlReceipt, String> {
    reconcile_runtime_server_activation_event(
        state_home,
        event,
        event.previous_artifact_digest.as_ref(),
        RuntimeServerActivationAuthority::OperatorStart,
        true,
    )
    .await
}

pub(crate) async fn ensure_healthy_runtime_server_for_client_recovery(
    state_home: &Path,
    event: &agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent,
) -> Result<agent_semantic_client_db::runtime_server_control::RuntimeServerControlReceipt, String> {
    reconcile_runtime_server_activation_event(
        state_home,
        event,
        event.previous_artifact_digest.as_ref(),
        RuntimeServerActivationAuthority::ClientBootstrap,
        false,
    )
    .await
}

pub(crate) async fn reconcile_runtime_server_activation_event(
    state_home: &Path,
    event: &agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent,
    serving_digest: Option<&agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest>,
    authority: RuntimeServerActivationAuthority,
    force_handoff: bool,
) -> Result<agent_semantic_client_db::runtime_server_control::RuntimeServerControlReceipt, String> {
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

    let previous_serving_digest = serving_digest.cloned();

    let environment = runtime_activation_environment(state_home, event);
    let owner_stderr_path = agent_semantic_artifacts::StateHomeLayout::new(state_home)
        .runtime_state()
        .serving()
        .owner_stderr_log();
    let request =
        agent_semantic_client_db::runtime_server_supervisor::SupervisorRequest::for_activation(
            state_home.to_owned(),
            event.artifact_path.clone(),
            event.publication_nonce.clone(),
            event.artifact_digest.clone(),
            previous_serving_digest,
            event.artifact_path.clone(),
            vec!["server".to_owned(), "daemon".to_owned()],
            None,
            environment,
            owner_stderr_path.clone(),
        );
    let mut supervision = if force_handoff {
        agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
            .restart_runtime_server_monitored(request)
            .await?
    } else {
        agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
            .ensure_runtime_server_monitored(request, authority.is_explicit_operator_start())
            .await?
    };
    let mut observed_transaction = None;
    if supervision.outcome
        == agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::SpawnAccepted
    {
        let mut process = supervision.process.take().ok_or_else(|| {
            "Runtime activation SpawnAccepted outcome requires a monitored child".to_owned()
        })?;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut last_observation = "canonical endpoint not yet published".to_owned();
        loop {
            tokio::select! {
                exit = process.wait() => {
                    let exit = exit?;
                    let daemon_stderr = tokio::fs::read_to_string(&owner_stderr_path)
                        .await
                        .unwrap_or_else(|error| format!("unavailable: {error}"));
                    return Err(serde_json::json!({
                        "schemaId": "agent.semantic-protocols.runtime-supervision-terminal",
                        "schemaVersion": "1",
                        "state": "failed",
                        "reasonKind": "runtime-owner-exited-before-healthy",
                        "exitStatus": exit.to_string(),
                        "daemonStderr": daemon_stderr,
                        "publicationNonce": event.publication_nonce,
                        "artifactDigest": event.artifact_digest,
                    }).to_string());
                }
                _ = tokio::time::sleep_until(deadline) => {
                    return Err(serde_json::json!({
                        "schemaId": "agent.semantic-protocols.runtime-supervision-terminal",
                        "schemaVersion": "1",
                        "state": "failed",
                        "reasonKind": "runtime-healthy-observation-timeout",
                        "lastObservation": last_observation,
                        "publicationNonce": event.publication_nonce,
                        "artifactDigest": event.artifact_digest,
                    }).to_string());
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {}
            }
            match crate::server::runtime_server::observe_runtime_server_readiness(state_home).await
            {
                Ok(receipt)
                    if receipt.state
                        == agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy =>
                {
                    match agent_semantic_client_db::runtime_server_lifecycle::observe_resident_transaction(
                        state_home,
                    )
                    .await
                    {
                        Ok(transaction)
                            if resident_transaction_identity_matches(
                                event,
                                &transaction.publication_nonce,
                                &transaction.applied_artifact_digest,
                            ) =>
                        {
                            observed_transaction = Some(transaction);
                            break;
                        }
                        Ok(_) => {
                            last_observation =
                                "healthy endpoint is not bound to the admitted activation transaction"
                                    .to_owned();
                        }
                        Err(error) => last_observation = error,
                    }
                }
                Ok(receipt) => {
                    last_observation = format!(
                        "state={:?} reason={}",
                        receipt.state,
                        receipt.reason.as_deref().unwrap_or("none")
                    );
                }
                Err(error) => last_observation = error,
            }
        }
    } else if supervision.outcome
        == agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::AlreadyResident
    {
        // A concurrent bootstrap may observe the winner's owner receipt before
        // that owner atomically publishes its healthy endpoint.  Joining that
        // exact activation transaction is the single-flight path; an immediate
        // endpoint read creates a false failure window and a second spawn would
        // create competing lifecycle authority.
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut last_observation = "resident owner has not published its endpoint".to_owned();
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(serde_json::json!({
                    "schemaId": "agent.semantic-protocols.runtime-supervision-terminal",
                    "schemaVersion": "1",
                    "state": "failed",
                    "reasonKind": "runtime-resident-healthy-observation-timeout",
                    "lastObservation": last_observation,
                    "publicationNonce": event.publication_nonce,
                    "artifactDigest": event.artifact_digest,
                })
                .to_string());
            }
            match crate::server::runtime_server::observe_runtime_server_readiness(state_home).await
            {
                Ok(receipt)
                    if receipt.state
                        == agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy =>
                {
                    match agent_semantic_client_db::runtime_server_lifecycle::observe_resident_transaction(
                        state_home,
                    )
                    .await
                    {
                        Ok(transaction)
                            if resident_transaction_identity_matches(
                                event,
                                &transaction.publication_nonce,
                                &transaction.applied_artifact_digest,
                            ) =>
                        {
                            observed_transaction = Some(transaction);
                            break;
                        }
                        Ok(_) => {
                            last_observation =
                                "healthy endpoint is not bound to the joined activation transaction"
                                    .to_owned();
                        }
                        Err(error) => last_observation = error,
                    }
                }
                Ok(receipt) => {
                    last_observation = format!(
                        "state={:?} reason={}",
                        receipt.state,
                        receipt.reason.as_deref().unwrap_or("none")
                    );
                }
                Err(error) => last_observation = error,
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
    let mut ready =
        crate::server::runtime_server::observe_runtime_server_readiness(state_home).await?;
    let transaction = match observed_transaction {
        Some(transaction) => transaction,
        None => {
            agent_semantic_client_db::runtime_server_lifecycle::observe_resident_transaction(
                state_home,
            )
            .await?
        }
    };
    if !resident_transaction_identity_matches(
        event,
        &transaction.publication_nonce,
        &transaction.applied_artifact_digest,
    ) {
        return Err(
            "state=runtime-activation-ready-failed reasonKind=transaction-authority-mismatch"
                .to_owned(),
        );
    }
    ready.resident_transaction = Some(transaction);
    Ok(ready)
}

#[cfg(test)]
#[path = "../../tests/unit/server/runtime_server_wire_adapter.rs"]
mod tests;
