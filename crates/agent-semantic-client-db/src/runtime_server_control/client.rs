use super::connection_pool::connection_pool;
use super::frame::{read_frame_sync, write_frame_sync};
use super::model::{
    RECEIPT_SCHEMA_ID, REQUEST_SCHEMA_ID, RuntimeServerControlReceipt, RuntimeServerControlRequest,
    RuntimeServerEndpoint, RuntimeServerOperation, SCHEMA_VERSION,
};
use super::status_memory::read_runtime_server_status;

/// Total wall budget for the synchronous Hook control-socket liveness probe.
pub const RUNTIME_SERVER_HOOK_CONTROL_PROBE_BUDGET: std::time::Duration =
    std::time::Duration::from_millis(75);

/// Failure class for a Hook liveness probe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeServerHookProbeError {
    Stale(String),
    LiveTransient(String),
    FailClosed(String),
}

impl std::fmt::Display for RuntimeServerHookProbeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stale(reason) | Self::LiveTransient(reason) | Self::FailClosed(reason) => {
                formatter.write_str(reason)
            }
        }
    }
}

const MAX_RUNTIME_SERVER_HOOK_CONNECT_WORKERS: usize = 16;
static RUNTIME_SERVER_HOOK_CONNECT_WORKERS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

struct RuntimeServerHookConnectWorkerLease;

impl RuntimeServerHookConnectWorkerLease {
    fn acquire() -> Result<Self, RuntimeServerHookProbeError> {
        use std::sync::atomic::Ordering;
        RUNTIME_SERVER_HOOK_CONNECT_WORKERS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < MAX_RUNTIME_SERVER_HOOK_CONNECT_WORKERS).then_some(active + 1)
            })
            .map_err(|_| {
                RuntimeServerHookProbeError::FailClosed(
                    "Runtime Server Hook connect worker limit reached".to_owned(),
                )
            })?;
        Ok(Self)
    }
}

impl Drop for RuntimeServerHookConnectWorkerLease {
    fn drop(&mut self) {
        RUNTIME_SERVER_HOOK_CONNECT_WORKERS.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}

/// Exchanges one Status frame on the discovered control socket.
pub fn probe_runtime_server_for_hook(
    endpoint: &RuntimeServerEndpoint,
    request_id: String,
) -> Result<RuntimeServerControlReceipt, RuntimeServerHookProbeError> {
    let started = std::time::Instant::now();
    endpoint
        .validate()
        .map_err(RuntimeServerHookProbeError::FailClosed)?;
    let request = RuntimeServerControlRequest {
        schema_id: REQUEST_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        operation: RuntimeServerOperation::Status,
        expected_runtime_artifact_digest: endpoint.runtime_artifact_digest.clone(),
        request_id: request_id.clone(),
        transport_contract_digest: endpoint.transport_contract_digest.clone(),
        owner_epoch: endpoint.owner_epoch,
        binding_token: endpoint.binding_token.clone(),
    };
    request
        .validate_for_endpoint(endpoint)
        .map_err(RuntimeServerHookProbeError::FailClosed)?;
    let socket_path = endpoint.socket_path.clone();
    let (connect_sender, connect_receiver) = std::sync::mpsc::sync_channel(1);
    let connect_worker_lease = RuntimeServerHookConnectWorkerLease::acquire()?;
    std::thread::Builder::new()
        .name("asp-hook-control-connect".to_owned())
        .spawn(move || {
            let _connect_worker_lease = connect_worker_lease;
            let _ = connect_sender.send(std::os::unix::net::UnixStream::connect(socket_path));
        })
        .map_err(|error| {
            RuntimeServerHookProbeError::FailClosed(format!(
                "failed to start Runtime Server Hook connect worker: {error}"
            ))
        })?;
    let connect_budget = RUNTIME_SERVER_HOOK_CONTROL_PROBE_BUDGET
        .checked_sub(started.elapsed())
        .ok_or_else(|| hook_probe_budget_exceeded(started.elapsed()))?;
    let mut stream = connect_receiver
        .recv_timeout(connect_budget)
        .map_err(|error| match error {
            std::sync::mpsc::RecvTimeoutError::Timeout => {
                hook_probe_budget_exceeded(started.elapsed())
            }
            std::sync::mpsc::RecvTimeoutError::Disconnected => {
                RuntimeServerHookProbeError::FailClosed(
                    "Runtime Server Hook connect worker disconnected".to_owned(),
                )
            }
        })?
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
                RuntimeServerHookProbeError::Stale(format!(
                    "Runtime Server control endpoint is stale: {error}"
                ))
            }
            _ => RuntimeServerHookProbeError::FailClosed(format!(
                "failed to connect Runtime Server control endpoint: {error}"
            )),
        })?;
    crate::runtime_server_control::validate_runtime_server_peer_fd(
        std::os::fd::AsRawFd::as_raw_fd(&stream),
    )
    .map_err(RuntimeServerHookProbeError::FailClosed)?;
    let write_budget = RUNTIME_SERVER_HOOK_CONTROL_PROBE_BUDGET
        .checked_sub(started.elapsed())
        .ok_or_else(|| hook_probe_budget_exceeded(started.elapsed()))?;
    stream
        .set_write_timeout(Some(write_budget))
        .map_err(|error| {
            RuntimeServerHookProbeError::FailClosed(format!(
                "failed to bound Runtime Server Hook control probe: {error}"
            ))
        })?;
    write_frame_sync(&mut stream, &[&request])
        .map_err(RuntimeServerHookProbeError::LiveTransient)?;
    let read_budget = RUNTIME_SERVER_HOOK_CONTROL_PROBE_BUDGET
        .checked_sub(started.elapsed())
        .ok_or_else(|| hook_probe_budget_exceeded(started.elapsed()))?;
    stream
        .set_read_timeout(Some(read_budget))
        .map_err(|error| {
            RuntimeServerHookProbeError::FailClosed(format!(
                "failed to bound Runtime Server Hook control probe: {error}"
            ))
        })?;
    let mut receipts: Vec<RuntimeServerControlReceipt> =
        read_frame_sync(&mut stream).map_err(RuntimeServerHookProbeError::LiveTransient)?;
    if receipts.len() != 1 {
        return Err(RuntimeServerHookProbeError::FailClosed(format!(
            "Runtime Server returned {} receipts for one Hook control probe",
            receipts.len()
        )));
    }
    let receipt = receipts.remove(0);
    if receipt.schema_id != RECEIPT_SCHEMA_ID
        || receipt.schema_version != SCHEMA_VERSION
        || receipt.request_id != request_id
        || receipt.runtime_artifact_digest != endpoint.runtime_artifact_digest
        || receipt.artifact_mode != endpoint.artifact_mode
        || receipt.artifact_catalog_digest != endpoint.artifact_catalog_digest
        || receipt.transport_contract_digest != endpoint.transport_contract_digest
    {
        return Err(RuntimeServerHookProbeError::FailClosed(
            "Runtime Server Hook control receipt identity mismatch".to_owned(),
        ));
    }
    Ok(receipt)
}

fn hook_probe_budget_exceeded(elapsed: std::time::Duration) -> RuntimeServerHookProbeError {
    RuntimeServerHookProbeError::LiveTransient(
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-server-hook-probe-wall-failure.v1",
            "schemaVersion": "1",
            "state": "unavailable",
            "reasonKind": "runtime-server-hook-probe-budget-exceeded",
            "executionBudgetMicros": RUNTIME_SERVER_HOOK_CONTROL_PROBE_BUDGET.as_micros(),
            "elapsedMicros": elapsed.as_micros(),
        })
        .to_string(),
    )
}

pub async fn call_runtime_server(
    endpoint: &RuntimeServerEndpoint,
    operation: RuntimeServerOperation,
    expected_runtime_artifact_digest: String,
    request_id: String,
) -> Result<RuntimeServerControlReceipt, String> {
    if operation == RuntimeServerOperation::Status {
        endpoint.validate()?;
    } else {
        endpoint.validate_supervisor_control()?;
    }
    if operation == RuntimeServerOperation::Status
        && expected_runtime_artifact_digest == endpoint.runtime_artifact_digest
    {
        return read_runtime_server_status(endpoint, request_id).await;
    }
    let request = RuntimeServerControlRequest {
        schema_id: REQUEST_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        operation,
        expected_runtime_artifact_digest,
        request_id: request_id.clone(),
        transport_contract_digest: endpoint.transport_contract_digest.clone(),
        owner_epoch: endpoint.owner_epoch,
        binding_token: endpoint.binding_token.clone(),
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
        expected_runtime_artifact_digest,
        request_id: request_id.clone(),
        transport_contract_digest: expected_transport_contract_digest,
        owner_epoch: endpoint.owner_epoch,
        binding_token: endpoint.binding_token.clone(),
    };
    let pool = connection_pool(endpoint).await;
    let receipt = pool.exchange(request).await?;
    if receipt.schema_id != RECEIPT_SCHEMA_ID
        || receipt.schema_version != SCHEMA_VERSION
        || receipt.request_id != request_id
        || receipt.runtime_artifact_digest != endpoint.runtime_artifact_digest
        || receipt.transport_contract_digest != endpoint.transport_contract_digest
    {
        return Err("Runtime Server reconcile receipt identity mismatch".to_owned());
    }
    if receipt.state == super::RuntimeServerState::Healthy {
        pool.prewarm().await?;
    }
    Ok(receipt)
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_control_hook_probe.rs"]
mod tests;
