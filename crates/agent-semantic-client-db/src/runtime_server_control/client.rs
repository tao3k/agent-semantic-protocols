use super::connection_pool::connection_pool;
use super::model::{
    RECEIPT_SCHEMA_ID, REQUEST_SCHEMA_ID, RuntimeServerControlReceipt, RuntimeServerControlRequest,
    RuntimeServerEndpoint, RuntimeServerOperation, SCHEMA_VERSION,
};
use super::status_memory::read_runtime_server_status;
use agent_semantic_runtime::runtime_artifact_catalog::RuntimeBinaryIdentity;
pub async fn call_runtime_server(
    endpoint: &RuntimeServerEndpoint,
    operation: RuntimeServerOperation,
    expected_runtime_binary_identity: RuntimeBinaryIdentity,
    request_id: String,
) -> Result<RuntimeServerControlReceipt, String> {
    if operation == RuntimeServerOperation::Status {
        endpoint.validate()?;
    } else {
        endpoint.validate_supervisor_control()?;
    }
    if operation == RuntimeServerOperation::Status
        && expected_runtime_binary_identity == endpoint.runtime_binary_identity
    {
        return read_runtime_server_status(endpoint, request_id).await;
    }
    let request = RuntimeServerControlRequest {
        schema_id: REQUEST_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        operation,
        expected_runtime_binary_identity,
        request_id: request_id.clone(),
        transport_contract_digest: endpoint.transport_contract_digest.clone(),
        owner_epoch: endpoint.owner_epoch,
        binding_token: endpoint.binding_token.clone(),
        project_root: None,
    };
    let pool = connection_pool(endpoint).await;
    let receipt = pool.exchange(request).await?;
    if operation == RuntimeServerOperation::Reconcile
        && receipt.state == super::RuntimeServerState::Healthy
    {
        pool.prewarm().await?;
    }
    if receipt.schema_id != RECEIPT_SCHEMA_ID
        || receipt.schema_version != SCHEMA_VERSION
        || receipt.request_id != request_id
        || receipt.transport_contract_digest != endpoint.transport_contract_digest
    {
        return Err("Runtime Server control receipt identity mismatch".to_owned());
    }
    Ok(receipt)
}

pub async fn ensure_runtime_server_workspace(
    endpoint: &RuntimeServerEndpoint,
    project_root: &std::path::Path,
    request_id: String,
) -> Result<RuntimeServerControlReceipt, String> {
    endpoint.validate_supervisor_control()?;
    if !project_root.is_absolute() {
        return Err(format!(
            "Runtime Server ensure-workspace project root must be absolute: {}",
            project_root.display()
        ));
    }
    static ENSURE_WORKSPACE_EXCHANGE_SEQUENCE: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(0);
    let sequence =
        ENSURE_WORKSPACE_EXCHANGE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let started_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("Runtime Server control clock is before Unix epoch: {error}"))?
        .as_nanos();
    let exchange_request_id = blake3::hash(
        format!(
            "runtime-server-ensure-workspace-exchange-v1\0{}\0{}\0{}\0{}",
            request_id,
            agent_semantic_runtime::runtime_process_lifecycle::current_process_id(),
            started_at,
            sequence,
        )
        .as_bytes(),
    )
    .to_hex()
    .to_string();
    let request = RuntimeServerControlRequest {
        schema_id: REQUEST_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        operation: RuntimeServerOperation::EnsureWorkspace,
        expected_runtime_binary_identity: endpoint.runtime_binary_identity.clone(),
        request_id: exchange_request_id.clone(),
        transport_contract_digest: endpoint.transport_contract_digest.clone(),
        owner_epoch: endpoint.owner_epoch,
        binding_token: endpoint.binding_token.clone(),
        project_root: Some(project_root.display().to_string()),
    };
    let receipt = connection_pool(endpoint).await.exchange(request).await?;
    if receipt.schema_id != RECEIPT_SCHEMA_ID
        || receipt.schema_version != SCHEMA_VERSION
        || receipt.request_id != exchange_request_id
        || receipt.transport_contract_digest != endpoint.transport_contract_digest
    {
        return Err("Runtime Server ensure-workspace receipt identity mismatch".to_owned());
    }
    if receipt.state != super::RuntimeServerState::Healthy {
        return Err(receipt
            .reason
            .clone()
            .unwrap_or_else(|| "Runtime Server ensure-workspace failed".to_owned()));
    }
    Ok(receipt)
}

pub async fn reconcile_runtime_server(
    endpoint: &RuntimeServerEndpoint,
    expected_runtime_artifact_digest: String,
    expected_transport_contract_digest: String,
    request_id: String,
) -> Result<RuntimeServerControlReceipt, String> {
    endpoint.validate_supervisor_control()?;
    if expected_transport_contract_digest.is_empty() {
        return Err("Runtime Server expected transport contract digest is empty".to_owned());
    }
    let request = RuntimeServerControlRequest {
        schema_id: REQUEST_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        operation: RuntimeServerOperation::Reconcile,
        expected_runtime_binary_identity:
            agent_semantic_runtime::runtime_artifact_catalog::RuntimeBinaryIdentity::Content {
                value: expected_runtime_artifact_digest,
                algorithm: "blake3-256".to_owned(),
            },
        request_id: request_id.clone(),
        transport_contract_digest: expected_transport_contract_digest,
        owner_epoch: endpoint.owner_epoch,
        binding_token: endpoint.binding_token.clone(),
        project_root: None,
    };
    let pool = connection_pool(endpoint).await;
    let receipt = pool.exchange(request).await?;
    if receipt.schema_id != RECEIPT_SCHEMA_ID
        || receipt.schema_version != SCHEMA_VERSION
        || receipt.request_id != request_id
        || receipt.runtime_binary_identity != endpoint.runtime_binary_identity
        || receipt.transport_contract_digest != endpoint.transport_contract_digest
    {
        return Err("Runtime Server reconcile receipt identity mismatch".to_owned());
    }
    if receipt.state == super::RuntimeServerState::Healthy {
        pool.prewarm().await?;
    }
    Ok(receipt)
}
