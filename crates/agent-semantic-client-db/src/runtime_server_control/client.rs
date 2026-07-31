use super::connection_pool::connection_pool;
use super::model::{
    RECEIPT_SCHEMA_ID, REQUEST_SCHEMA_ID, RuntimeServerControlReceipt, RuntimeServerControlRequest,
    RuntimeServerEndpoint, RuntimeServerOperation, SCHEMA_VERSION,
};
use super::status_memory::read_runtime_server_status;

pub async fn call_runtime_server(
    endpoint: &RuntimeServerEndpoint,
    operation: RuntimeServerOperation,
    expected_runtime_artifact_digest: String,
    request_id: String,
) -> Result<RuntimeServerControlReceipt, String> {
    endpoint.validate()?;
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
    if operation == RuntimeServerOperation::Reconcile {
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
