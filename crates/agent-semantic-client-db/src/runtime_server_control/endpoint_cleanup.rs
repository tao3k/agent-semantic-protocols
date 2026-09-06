// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

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
    if let (Some(owner_epoch), Some(binding_token), Some(runtime_binary_digest)) = (
        value.get("ownerEpoch").and_then(serde_json::Value::as_u64),
        value
            .get("bindingToken")
            .and_then(serde_json::Value::as_str),
        value
            .get("runtimeBinaryIdentity")
            .and_then(|value| value.get("value"))
            .and_then(serde_json::Value::as_str),
    ) {
        let runtime_base = super::endpoint_identity::runtime_server_runtime_base(state_home)?;
        paths.push(
            super::endpoint_identity::runtime_server_status_memory_path_for_identity(
                &runtime_base,
                owner_epoch,
                binding_token,
                runtime_binary_digest,
            ),
        );
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

#[cfg(test)]
mod tests {
    use super::cleanup_invalid_runtime_server_endpoint;

    #[tokio::test]
    async fn invalid_endpoint_cleanup_uses_its_content_bound_status_identity() {
        let fixture = tempfile::tempdir().expect("invalid endpoint cleanup fixture");
        let state_home = fixture.path().join("state");
        tokio::fs::create_dir_all(&state_home)
            .await
            .expect("create State Home");
        let runtime_base =
            super::super::endpoint_identity::runtime_server_runtime_base(&state_home)
                .expect("derive Runtime serving root");
        tokio::fs::create_dir_all(&runtime_base)
            .await
            .expect("create Runtime serving root");
        let endpoint_path =
            super::super::endpoint_identity::runtime_server_endpoint_path(&state_home)
                .expect("derive endpoint path");
        let owner_epoch = 7_u64;
        let binding_token = "binding-current";
        let runtime_binary_digest = format!("blake3-256:{}", "a".repeat(64));
        let status_identity = blake3::hash(
            format!("{owner_epoch}\0{binding_token}\0{runtime_binary_digest}").as_bytes(),
        )
        .to_hex();
        let status_path = runtime_base.join(format!("status-{}.memory", &status_identity[..16]));
        let unrelated_status_path = runtime_base.join("status-unrelated.memory");
        tokio::fs::write(&status_path, b"stale status")
            .await
            .expect("write content-bound status");
        tokio::fs::write(&unrelated_status_path, b"unrelated status")
            .await
            .expect("write unrelated status");
        tokio::fs::write(
            &endpoint_path,
            serde_json::to_vec(&serde_json::json!({
                "ownerEpoch": owner_epoch,
                "bindingToken": binding_token,
                "runtimeBinaryIdentity": { "value": runtime_binary_digest },
                "invalid": true
            }))
            .expect("encode invalid endpoint"),
        )
        .await
        .expect("write invalid endpoint");

        cleanup_invalid_runtime_server_endpoint(&state_home)
            .await
            .expect("clean invalid endpoint");

        assert!(
            !endpoint_path.exists(),
            "invalid endpoint receipt is removed"
        );
        assert!(
            !status_path.exists(),
            "the content-bound stale status memory is removed"
        );
        assert!(
            unrelated_status_path.exists(),
            "cleanup cannot remove another identity's status memory"
        );
    }
}
