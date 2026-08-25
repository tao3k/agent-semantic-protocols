use super::endpoint_identity::runtime_server_endpoint_path;
use super::model::RuntimeServerEndpoint;
use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub async fn read_endpoint(path: &Path) -> Result<RuntimeServerEndpoint, String> {
    let endpoint = read_supervisor_endpoint(path).await?;
    endpoint.validate()?;
    validate_runtime_server_generation(&endpoint, &endpoint.binary_content_digest)?;
    Ok(endpoint)
}

pub fn validate_runtime_server_generation(
    endpoint: &RuntimeServerEndpoint,
    expected_binary_content_digest: &str,
) -> Result<(), String> {
    let expected =
        agent_semantic_client_protocol::runtime_generation::RuntimeServerGenerationIdentity::derive(
            expected_binary_content_digest,
            &endpoint.schema_id,
            &endpoint.schema_version,
            &endpoint.transport_contract_digest,
            &endpoint.artifact_catalog_digest,
            endpoint.owner_epoch,
        );
    let observed =
        agent_semantic_client_protocol::runtime_generation::RuntimeServerGenerationIdentity {
            binary_content_digest: endpoint.binary_content_digest.clone(),
            runtime_generation_digest: endpoint.runtime_generation_digest.clone(),
            schema_digest: endpoint.schema_digest.clone(),
        };
    expected
        .validate(&observed)
        .map_err(|error| error.to_string())
}

pub async fn read_supervisor_endpoint(path: &Path) -> Result<RuntimeServerEndpoint, String> {
    let bytes = tokio::fs::read(path).await.map_err(|error| {
        format!(
            "Runtime Server endpoint is unavailable at {}: {error}",
            path.display()
        )
    })?;
    let mut value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to decode Runtime Server endpoint: {error}"))?;
    let migrated = materialize_endpoint_v1_identity(&mut value)?;
    let endpoint: RuntimeServerEndpoint = serde_json::from_value(value)
        .map_err(|error| format!("failed to decode Runtime Server endpoint: {error}"))?;
    endpoint.validate_supervisor_control()?;
    if migrated {
        rewrite_complete_endpoint_v1(path, &endpoint).await?;
    }
    Ok(endpoint)
}

fn materialize_endpoint_v1_identity(value: &mut Value) -> Result<bool, String> {
    let object = value.as_object_mut().ok_or_else(|| {
        "reasonKind=runtime-server-endpoint-identity-incomplete endpoint v1 is not an object"
            .to_owned()
    })?;
    let missing = [
        "binaryContentDigest",
        "runtimeGenerationDigest",
        "schemaDigest",
    ]
    .iter()
    .any(|field| !object.contains_key(*field));
    if !missing {
        return Ok(false);
    }

    let required_string = |field: &str| -> Result<String, String> {
        object
            .get(field)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                format!(
                    "reasonKind=runtime-server-endpoint-identity-incomplete missing authoritative field {field}"
                )
            })
    };
    let runtime_identity: RuntimeBinaryIdentity = serde_json::from_value(
        object
            .get("runtimeBinaryIdentity")
            .cloned()
            .ok_or_else(|| {
                "reasonKind=runtime-server-endpoint-identity-incomplete missing runtimeBinaryIdentity"
                    .to_owned()
            })?,
    )
    .map_err(|error| {
        format!(
            "reasonKind=runtime-server-endpoint-identity-incomplete invalid runtimeBinaryIdentity: {error}"
        )
    })?;
    let binary_content_digest = runtime_identity.value().to_owned();
    if binary_content_digest.is_empty() {
        return Err(
            "reasonKind=runtime-server-endpoint-identity-incomplete empty canonical binary identity"
                .to_owned(),
        );
    }
    let owner_process_id = object
        .get("ownerProcessId")
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            "reasonKind=runtime-server-endpoint-identity-incomplete missing ownerProcessId"
                .to_owned()
        })?;
    let owner_epoch = object
        .get("ownerEpoch")
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            "reasonKind=runtime-server-endpoint-identity-incomplete missing ownerEpoch".to_owned()
        })?;
    let binding_token = required_string("bindingToken")?;
    let socket_path = required_string("socketPath")?;
    let runtime_artifact_path = required_string("runtimeArtifactPath")?;
    let schema_id = required_string("schemaId")?;
    let schema_version = required_string("schemaVersion")?;
    let transport_contract_digest = required_string("transportContractDigest")?;

    let runtime_generation_digest = canonical_endpoint_digest(
        "runtime-generation",
        &[
            &binary_content_digest,
            &owner_process_id.to_string(),
            &owner_epoch.to_string(),
            &binding_token,
            &socket_path,
            &runtime_artifact_path,
        ],
    );
    let schema_digest = canonical_endpoint_digest(
        "endpoint-schema",
        &[&schema_id, &schema_version, &transport_contract_digest],
    );
    object.insert(
        "binaryContentDigest".to_owned(),
        Value::String(binary_content_digest),
    );
    object.insert(
        "runtimeGenerationDigest".to_owned(),
        Value::String(runtime_generation_digest),
    );
    object.insert("schemaDigest".to_owned(), Value::String(schema_digest));
    Ok(true)
}

fn canonical_endpoint_digest(label: &str, fields: &[&str]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(label.as_bytes());
    for field in fields {
        hasher.update(&[0]);
        hasher.update(field.as_bytes());
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

async fn rewrite_complete_endpoint_v1(
    path: &Path,
    endpoint: &RuntimeServerEndpoint,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(endpoint)
        .map_err(|error| format!("failed to encode migrated Runtime Server endpoint: {error}"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("endpoint.v1.json");
    let temporary = path.with_file_name(format!(
        ".{file_name}.migrate-{}-{}.tmp",
        std::process::id(),
        blake3::hash(&bytes).to_hex()
    ));
    tokio::fs::write(&temporary, &bytes)
        .await
        .map_err(|error| {
            format!(
                "failed to stage migrated Runtime Server endpoint {}: {error}",
                temporary.display()
            )
        })?;
    tokio::fs::rename(&temporary, path).await.map_err(|error| {
        format!(
            "failed to atomically publish migrated Runtime Server endpoint {}: {error}",
            path.display()
        )
    })?;
    Ok(())
}

pub async fn remove_stale_socket(path: &Path) -> Result<(), String> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove stale Runtime Server socket {}: {error}",
            path.display()
        )),
    }
}

pub async fn cleanup_endpoint(
    state_home: &Path,
    endpoint: &RuntimeServerEndpoint,
) -> Result<(), String> {
    let endpoint_path = runtime_server_endpoint_path(state_home)?;
    let socket_path = PathBuf::from(&endpoint.socket_path);
    let data_plane_socket_path = PathBuf::from(&endpoint.data_plane_socket_path);
    let status_memory_path = PathBuf::from(&endpoint.status_memory_path);
    let owned = read_endpoint(&endpoint_path).await.is_ok_and(|actual| {
        actual.owner_epoch == endpoint.owner_epoch && actual.binding_token == endpoint.binding_token
    });
    if owned {
        let _ = tokio::fs::remove_file(endpoint_path).await;
        let _ = tokio::fs::remove_file(socket_path).await;
        let _ = tokio::fs::remove_file(data_plane_socket_path).await;
        let _ = tokio::fs::remove_file(status_memory_path).await;
    }
    Ok(())
}
