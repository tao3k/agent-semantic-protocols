use std::path::{Path, PathBuf};

use super::endpoint::read_runtime_server_supervisor_endpoint;
use super::endpoint_identity::runtime_server_endpoint_path_async;
use super::model::RuntimeServerEndpoint;

pub async fn cleanup_runtime_server_endpoint(
    state_home: &Path,
    endpoint: &RuntimeServerEndpoint,
) -> Result<(), String> {
    let Some(actual) = read_runtime_server_supervisor_endpoint(state_home).await? else {
        return Ok(());
    };
    if actual.owner_epoch != endpoint.owner_epoch || actual.binding_token != endpoint.binding_token
    {
        return Err("Runtime Server endpoint cleanup ownership mismatch".to_owned());
    }
    for path in [
        runtime_server_endpoint_path_async(state_home).await?,
        PathBuf::from(&endpoint.status_memory_path),
    ] {
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "remove Runtime Server artifact {}: {error}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

/// Removes artifacts named by a receipt that cannot pass strict decoding. This is
/// only called after owner classification proves the recorded owner is stale.
pub(crate) async fn cleanup_invalid_runtime_server_endpoint(
    state_home: &Path,
) -> Result<(), String> {
    let endpoint_path = runtime_server_endpoint_path_async(state_home).await?;
    let bytes = match tokio::fs::read(&endpoint_path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("read invalid Runtime Server endpoint: {error}")),
    };
    let value = match serde_json::from_slice::<serde_json::Value>(&bytes) {
        Ok(value) => value,
        Err(_) => {
            match tokio::fs::remove_file(&endpoint_path).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "remove invalid Runtime Server endpoint {}: {error}",
                        endpoint_path.display()
                    ));
                }
            }
            return Ok(());
        }
    };
    let mut paths = Vec::new();
    if value
        .get("ownerEpoch")
        .and_then(serde_json::Value::as_u64)
        .is_some()
        && value
            .get("bindingToken")
            .and_then(serde_json::Value::as_str)
            .is_some()
        && value
            .get("runtimeBinaryIdentity")
            .and_then(|value| value.get("value"))
            .and_then(serde_json::Value::as_str)
            .is_some()
    {
        let runtime_base = super::endpoint_identity::runtime_server_runtime_base(state_home)?;
        paths.push(runtime_base.join("status.v1.memory"));
    } else {
        // Without a verifiable identity, only remove the canonical receipt.
        // Never trust paths supplied by an invalid receipt.
    }
    paths.push(endpoint_path);
    for path in paths {
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "remove stale Runtime Server artifact {}: {error}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}
