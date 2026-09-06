//! Runtime Server lifecycle receipt store owned by client-db composition.

use crate::{
    RuntimeServerDrainReceipt, RuntimeServerExitReceipt, RuntimeServerResidentTransactionReceipt,
    RuntimeServerSpawnReceipt, RuntimeServerSpawnReceiptRead,
};
use std::path::{Path, PathBuf};

fn server_layout(home: &Path) -> agent_semantic_artifacts::RuntimeServingStateLayout {
    if let Some(publication_dir) = std::env::var_os("ASP_RUNTIME_SERVER_PUBLICATION_DIR") {
        return agent_semantic_artifacts::RuntimeServingStateLayout::from_injected_publication_root(
            publication_dir,
        );
    }
    agent_semantic_artifacts::StateHomeLayout::new(home)
        .runtime_state()
        .serving()
}
fn marker(home: &Path, name: agent_semantic_artifacts::RuntimeLifecycleReceiptName) -> PathBuf {
    server_layout(home).lifecycle_receipt(name)
}

pub fn spawn_receipt_digest(
    receipt: &RuntimeServerSpawnReceipt,
) -> Result<agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest, String> {
    let bytes = serde_json::to_vec(receipt)
        .map_err(|error| format!("encode Runtime owner-spawn receipt identity: {error}"))?;
    Ok(agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(&bytes))
}

async fn atomic_empty(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "lifecycle marker has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| e.to_string())?;
    let staged = path.with_extension(format!(
        "stage-{}",
        agent_semantic_runtime::runtime_process_lifecycle::current_process_id()
    ));
    let file = tokio::fs::File::create(&staged)
        .await
        .map_err(|e| e.to_string())?;
    file.sync_all().await.map_err(|e| e.to_string())?;
    drop(file);
    tokio::fs::rename(staged, path)
        .await
        .map_err(|e| e.to_string())
}

pub async fn create_run_intent(home: &Path) -> Result<(), String> {
    atomic_empty(&marker(
        home,
        agent_semantic_artifacts::RuntimeLifecycleReceiptName::RunIntent,
    ))
    .await
}
pub async fn remove_run_intent(home: &Path) -> Result<(), String> {
    remove_if_present(&marker(
        home,
        agent_semantic_artifacts::RuntimeLifecycleReceiptName::RunIntent,
    ))
    .await
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerOperatorStopReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub state: String,
    pub stopped_artifact_digest:
        Option<agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest>,
    pub stopped_publication_nonce: Option<String>,
}

pub async fn mark_operator_stopped(home: &Path) -> Result<(), String> {
    let _mutation_guard =
        agent_semantic_artifacts::runtime_artifact_retention::RuntimeArtifactMutationGuard::try_acquire(
            agent_semantic_artifacts::RuntimeArtifactStateLayout::new(home).root(),
        )?;
    let path = marker(
        home,
        agent_semantic_artifacts::RuntimeLifecycleReceiptName::OperatorStop,
    );
    let parent = path
        .parent()
        .ok_or_else(|| "operator marker has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| e.to_string())?;
    let staged = path.with_extension(format!(
        "stage-{}",
        agent_semantic_runtime::runtime_process_lifecycle::current_process_id()
    ));
    let stopped_publication = if let Some(pending) =
        agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(
            home,
        )
        .await?
    {
        Some(pending)
    } else {
        agent_semantic_artifacts::runtime_artifact_activation::
            read_applied_runtime_artifact_activation_event(home)
            .await?
    };
    let bytes = serde_json::to_vec(&RuntimeServerOperatorStopReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-operator-stop".to_owned(),
        schema_version: "1".to_owned(),
        state: "stopped".to_owned(),
        stopped_artifact_digest: stopped_publication
            .as_ref()
            .map(|event| event.artifact_digest.clone()),
        stopped_publication_nonce: stopped_publication.map(|event| event.publication_nonce),
    })
    .map_err(|e| e.to_string())?;
    tokio::fs::write(&staged, bytes)
        .await
        .map_err(|e| e.to_string())?;
    tokio::fs::rename(staged, path)
        .await
        .map_err(|e| e.to_string())
}

pub async fn clear_operator_stopped(home: &Path) -> Result<(), String> {
    let _mutation_guard =
        agent_semantic_artifacts::runtime_artifact_retention::RuntimeArtifactMutationGuard::try_acquire(
            agent_semantic_artifacts::RuntimeArtifactStateLayout::new(home).root(),
        )?;
    remove_if_present(&marker(
        home,
        agent_semantic_artifacts::RuntimeLifecycleReceiptName::OperatorStop,
    ))
    .await
}
pub async fn operator_stopped(home: &Path) -> Result<bool, String> {
    Ok(read_operator_stop_receipt(home).await?.is_some())
}

pub async fn read_operator_stop_receipt(
    home: &Path,
) -> Result<Option<RuntimeServerOperatorStopReceipt>, String> {
    read_operator_stop_receipt_locked(home).await
}

async fn read_operator_stop_receipt_locked(
    home: &Path,
) -> Result<Option<RuntimeServerOperatorStopReceipt>, String> {
    let path = marker(
        home,
        agent_semantic_artifacts::RuntimeLifecycleReceiptName::OperatorStop,
    );
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let value = serde_json::from_slice::<serde_json::Value>(&bytes)
        .map_err(|error| format!("invalid Runtime Server operator-stop receipt: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "invalid Runtime Server operator-stop receipt shape".to_owned())?;
    if object.get("schemaId").and_then(serde_json::Value::as_str)
        != Some("agent.semantic-protocols.runtime-server-operator-stop")
        || object.get("state").and_then(serde_json::Value::as_str) != Some("stopped")
    {
        return Err("invalid Runtime Server operator-stop receipt identity".to_owned());
    }
    match object
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
    {
        Some("1")
            if object.contains_key("stoppedArtifactDigest")
                && object.contains_key("stoppedPublicationNonce") => {}
        Some("1") => {
            return Err(
                "Runtime Server operator-stop v1 receipt is missing active bundle identity"
                    .to_owned(),
            );
        }
        Some("2") => {
            return Err(
                "unreleased Runtime Server operator-stop schemaVersion=2 is not current".to_owned(),
            );
        }
        Some(version) => {
            return Err(format!(
                "unsupported Runtime Server operator-stop schemaVersion={version}"
            ));
        }
        None => {
            return Err("Runtime Server operator-stop receipt is missing schemaVersion".to_owned());
        }
    }
    let receipt = serde_json::from_value::<RuntimeServerOperatorStopReceipt>(value)
        .map_err(|error| format!("invalid Runtime Server operator-stop receipt: {error}"))?;
    if receipt.schema_id != "agent.semantic-protocols.runtime-server-operator-stop"
        || receipt.schema_version != "1"
        || receipt.state != "stopped"
    {
        return Err("invalid Runtime Server operator-stop receipt identity".to_owned());
    }
    Ok(Some(receipt))
}

pub async fn admit_activation_after_operator_stop(
    home: &Path,
    artifact_digest: &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    publication_nonce: &str,
) -> Result<bool, String> {
    if !tokio::fs::try_exists(marker(
        home,
        agent_semantic_artifacts::RuntimeLifecycleReceiptName::OperatorStop,
    ))
    .await
    .map_err(|error| error.to_string())?
    {
        return Ok(true);
    }
    let _mutation_guard =
        agent_semantic_artifacts::runtime_artifact_retention::RuntimeArtifactMutationGuard::try_acquire(
            agent_semantic_artifacts::RuntimeArtifactStateLayout::new(home).root(),
        )?;
    let Some(receipt) = read_operator_stop_receipt_locked(home).await? else {
        return Ok(true);
    };
    if receipt.stopped_artifact_digest.as_ref() == Some(artifact_digest)
        && receipt.stopped_publication_nonce.as_deref() == Some(publication_nonce)
    {
        return Ok(false);
    }
    remove_if_present(&marker(
        home,
        agent_semantic_artifacts::RuntimeLifecycleReceiptName::OperatorStop,
    ))
    .await?;
    Ok(true)
}

fn owner_receipt(home: &Path) -> PathBuf {
    marker(
        home,
        agent_semantic_artifacts::RuntimeLifecycleReceiptName::OwnerSpawn,
    )
}
pub async fn read_owner_receipt(home: &Path) -> Result<Option<RuntimeServerSpawnReceipt>, String> {
    match read_owner_receipt_state(home).await? {
        Some(RuntimeServerSpawnReceiptRead::Current(receipt)) => Ok(Some(receipt)),
        Some(RuntimeServerSpawnReceiptRead::Stale(stale)) => Err(serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-authority-observation",
            "schemaVersion": "1",
            "state": "stale",
            "reasonKind": "runtime-authority-stale",
            "observedSchemaId": stale.schema_id,
            "observedSchemaVersion": stale.schema_version,
            "observationReasonKind": stale.reason_kind,
            "recommendedNext": "publish-validated-runtime-activation",
        })
        .to_string()),
        None => Ok(None),
    }
}

pub async fn read_owner_receipt_state(
    home: &Path,
) -> Result<Option<RuntimeServerSpawnReceiptRead>, String> {
    match tokio::fs::read(owner_receipt(home)).await {
        Ok(bytes) => {
            crate::runtime_server_owner_receipt::decode_runtime_server_spawn_receipt(&bytes)
                .map(Some)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
pub async fn write_owner_receipt(
    home: &Path,
    receipt: &RuntimeServerSpawnReceipt,
) -> Result<(), String> {
    let path = owner_receipt(home);
    let parent = path
        .parent()
        .ok_or_else(|| "owner receipt has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| e.to_string())?;
    let staged = path.with_extension(format!(
        "stage-{}",
        agent_semantic_runtime::runtime_process_lifecycle::current_process_id()
    ));
    tokio::fs::write(
        &staged,
        serde_json::to_vec(receipt).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())?;
    tokio::fs::rename(staged, path)
        .await
        .map_err(|e| e.to_string())
}
pub async fn remove_owner_receipt(home: &Path) -> Result<(), String> {
    remove_if_present(&owner_receipt(home)).await
}

fn exit_receipt(home: &Path) -> PathBuf {
    marker(
        home,
        agent_semantic_artifacts::RuntimeLifecycleReceiptName::DaemonExit,
    )
}
fn drain_receipt(home: &Path) -> PathBuf {
    marker(
        home,
        agent_semantic_artifacts::RuntimeLifecycleReceiptName::DaemonDrain,
    )
}
pub async fn publish_drain(home: &Path, receipt: RuntimeServerDrainReceipt) -> Result<(), String> {
    let path = drain_receipt(home);
    let parent = path
        .parent()
        .ok_or_else(|| "drain receipt has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| e.to_string())?;
    let staged = path.with_extension(format!(
        "stage-{}",
        agent_semantic_runtime::runtime_process_lifecycle::current_process_id()
    ));
    tokio::fs::write(
        &staged,
        serde_json::to_vec(&receipt).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())?;
    tokio::fs::rename(staged, path)
        .await
        .map_err(|e| e.to_string())
}

pub async fn read_latest_drain(home: &Path) -> Result<Option<RuntimeServerDrainReceipt>, String> {
    match tokio::fs::read(drain_receipt(home)).await {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| format!("invalid Runtime Server drain receipt: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

struct CurrentRuntimeServingIdentity {
    spawn: RuntimeServerSpawnReceipt,
    applied: agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent,
    endpoint: crate::runtime_server_control::RuntimeServerEndpoint,
    endpoint_binary_content_digest:
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
}

/// Content-proven Runtime publication consumed by normal clients.
///
/// Keeping the publication nonce beside the endpoint prevents a client cache
/// from reusing a session across an otherwise byte-identical atomic switch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeServerServingPublication {
    pub endpoint: crate::runtime_server_control::RuntimeServerEndpoint,
    pub publication_nonce: String,
    pub artifact_digest: agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
}

/// Resolves the Runtime endpoint only after proving that the published owner,
/// applied artifact, and endpoint name the same serving artifact.  This is a
/// pure receipt check: it deliberately does not connect to the endpoint.
async fn resolve_current_runtime_serving_identity(
    home: &Path,
) -> Result<CurrentRuntimeServingIdentity, String> {
    let spawn = read_owner_receipt(home)
        .await?
        .ok_or_else(|| "Runtime serving endpoint requires an owner-spawn receipt".to_owned())?;
    let applied = agent_semantic_artifacts::runtime_artifact_activation::
        read_applied_runtime_artifact_activation_event(home)
        .await?
        .ok_or_else(|| "Runtime serving endpoint requires an applied activation".to_owned())?;
    let endpoint = crate::runtime_server_control::read_runtime_server_supervisor_endpoint(home)
        .await?
        .ok_or_else(|| "Runtime serving endpoint requires a published endpoint".to_owned())?;

    if spawn.publication_nonce != applied.publication_nonce
        || spawn.launcher_artifact_digest != applied.artifact_digest
        || spawn.launcher_artifact_path != applied.artifact_path.display().to_string()
    {
        return Err(
            "Runtime resident transaction launcher and applied activation identities differ"
                .to_owned(),
        );
    }
    if spawn.process_id != endpoint.owner_process_id
        || spawn.launcher_artifact_path != endpoint.runtime_artifact_path
    {
        return Err(
            "Runtime resident transaction launcher and endpoint owner identities differ".to_owned(),
        );
    }
    let endpoint_binary_content_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(
            &endpoint.binary_content_digest,
        )
        .map_err(|error| format!("invalid Runtime endpoint binary content digest: {error}"))?;
    if endpoint_binary_content_digest != applied.artifact_digest {
        return Err(
            "Runtime resident transaction applied and endpoint artifact digests differ".to_owned(),
        );
    }
    if spawn.previous_serving_digest != applied.previous_artifact_digest {
        return Err("Runtime serving endpoint previous serving identities differ".to_owned());
    }

    Ok(CurrentRuntimeServingIdentity {
        spawn,
        applied,
        endpoint,
        endpoint_binary_content_digest,
    })
}

/// Resolves the content-proven Runtime serving endpoint for a normal client.
/// Socket connection is intentionally a separate operation so an OS-level
/// refusal remains a typed transport result rather than a lifecycle failure.
pub async fn resolve_runtime_server_serving_endpoint(
    home: &Path,
) -> Result<crate::runtime_server_control::RuntimeServerEndpoint, String> {
    Ok(resolve_runtime_server_serving_publication(home)
        .await?
        .endpoint)
}

/// Resolves the complete serving publication after jointly validating the
/// Runtime owner, applied activation, artifact digest, endpoint, and nonce.
pub async fn resolve_runtime_server_serving_publication(
    home: &Path,
) -> Result<RuntimeServerServingPublication, String> {
    let current = resolve_current_runtime_serving_identity(home).await?;
    Ok(RuntimeServerServingPublication {
        publication_nonce: current.applied.publication_nonce.clone(),
        artifact_digest: current.endpoint_binary_content_digest.clone(),
        endpoint: current.endpoint,
    })
}

pub async fn observe_resident_transaction(
    home: &Path,
) -> Result<RuntimeServerResidentTransactionReceipt, String> {
    Ok(observe_resident_transaction_with_endpoint(home).await?.0)
}

/// Observe one content-bound Runtime transaction and retain the exact endpoint
/// value validated in that same observation.  Callers must not resolve or read
/// an endpoint path again after this function returns: doing so would create a
/// TOCTOU boundary between handoff admission and transport use.
pub async fn observe_resident_transaction_with_endpoint(
    home: &Path,
) -> Result<
    (
        RuntimeServerResidentTransactionReceipt,
        crate::runtime_server_control::RuntimeServerEndpoint,
    ),
    String,
> {
    let CurrentRuntimeServingIdentity {
        spawn,
        applied,
        endpoint,
        endpoint_binary_content_digest,
    } = resolve_current_runtime_serving_identity(home).await?;
    endpoint.validate_service_reachability().await?;

    let previous_drain_state = match spawn.previous_owner_epoch {
        Some(previous_owner_epoch) => {
            let drain = read_latest_drain(home).await?.ok_or_else(|| {
                format!(
                    "Runtime resident transaction is missing drain receipt for previous owner epoch {previous_owner_epoch}"
                )
            })?;
            if drain.owner_epoch != previous_owner_epoch || !drain.clean_drain {
                return Err(format!(
                    "Runtime resident transaction previous owner did not drain cleanly: expectedEpoch={previous_owner_epoch} observedEpoch={} cleanDrain={}",
                    drain.owner_epoch, drain.clean_drain
                ));
            }
            "clean"
        }
        None if spawn.previous_serving_digest.is_some() => "not-running",
        None => "not-required",
    };

    let receipt = RuntimeServerResidentTransactionReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-resident-transaction-receipt"
            .to_owned(),
        schema_version: "1".to_owned(),
        state: "ready".to_owned(),
        publication_nonce: spawn.publication_nonce,
        launcher_artifact_path: spawn.launcher_artifact_path,
        launcher_artifact_digest: spawn.launcher_artifact_digest,
        spawn_argv: spawn.spawn_argv,
        applied_artifact_digest: applied.artifact_digest,
        applied_publication_nonce: applied.publication_nonce,
        endpoint_owner_epoch: endpoint.owner_epoch,
        endpoint_binary_content_digest,
        endpoint_runtime_generation_digest: endpoint.runtime_generation_digest.clone(),
        control_endpoint: endpoint.control_endpoint.clone(),
        data_endpoint: endpoint.data_endpoint.clone(),
        provider_endpoint: endpoint.provider_endpoint.clone(),
        previous_serving_digest: applied.previous_artifact_digest,
        previous_owner_epoch: spawn.previous_owner_epoch,
        previous_drain_state: previous_drain_state.to_owned(),
    };
    Ok((receipt, endpoint))
}
pub async fn publish_with_errors(
    home: &Path,
    owner_epoch: u64,
    clean_drain: bool,
    errors: Vec<String>,
) -> Result<(), String> {
    let receipt = RuntimeServerExitReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-daemon-exit".to_owned(),
        schema_version: "1".to_owned(),
        owner_epoch,
        clean_drain,
        errors,
    };
    let path = exit_receipt(home);
    let parent = path
        .parent()
        .ok_or_else(|| "exit receipt has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| e.to_string())?;
    let staged = path.with_extension(format!(
        "stage-{}",
        agent_semantic_runtime::runtime_process_lifecycle::current_process_id()
    ));
    tokio::fs::write(
        &staged,
        serde_json::to_vec(&receipt).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())?;
    tokio::fs::rename(staged, path)
        .await
        .map_err(|e| e.to_string())
}
pub async fn read_latest_owner_exit(
    home: &Path,
) -> Result<Option<RuntimeServerExitReceipt>, String> {
    match tokio::fs::read(exit_receipt(home)).await {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Read the fixed exit marker only when it belongs to the currently expected owner.
/// A stale marker is evidence, not a terminal result for a new owner.
pub async fn read_owner_exit_for(
    home: &Path,
    expected_owner_epoch: u64,
) -> Result<Option<RuntimeServerExitReceipt>, String> {
    match read_latest_owner_exit(home).await? {
        Some(receipt) if receipt.owner_epoch == expected_owner_epoch => Ok(Some(receipt)),
        Some(receipt) => {
            eprintln!(
                "{}",
                serde_json::json!({
                    "schemaId": "agent.semantic-protocols.runtime-server-daemon-exit-stale",
                    "schemaVersion": "1",
                    "state": "ignored",
                    "expectedOwnerEpoch": expected_owner_epoch,
                    "observedOwnerEpoch": receipt.owner_epoch,
                    "reasonKind": "stale-owner-exit-receipt",
                })
            );
            Ok(None)
        }
        None => Ok(None),
    }
}
pub async fn remove_stale(home: &Path) -> Result<(), String> {
    remove_if_present(&exit_receipt(home)).await
}
pub async fn await_owner_exit(
    home: &Path,
    owner_epoch: u64,
) -> Result<RuntimeServerExitReceipt, String> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Some(receipt) = read_owner_exit_for(home, owner_epoch).await? {
            return Ok(receipt);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err("Runtime Server owner exit receipt timeout".to_owned());
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}

async fn remove_if_present(path: &Path) -> Result<(), String> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
