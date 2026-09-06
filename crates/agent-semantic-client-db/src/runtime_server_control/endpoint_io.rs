// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::endpoint_identity::runtime_server_endpoint_path;
use super::model::RuntimeServerEndpoint;
use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;
use serde_json::Value;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

unsafe extern "C" {
    fn getuid() -> u32;
}

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
    canonicalize_endpoint_file_security(path).await?;
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

async fn canonicalize_endpoint_file_security(path: &Path) -> Result<(), String> {
    let before = tokio::fs::symlink_metadata(path).await.map_err(|error| {
        format!(
            "failed to inspect Runtime Server endpoint {}: {error}",
            path.display()
        )
    })?;
    let current_uid = unsafe { getuid() };
    if !before.file_type().is_file()
        || before.file_type().is_symlink()
        || before.uid() != current_uid
        || before.mode() & 0o022 != 0
    {
        return Err(format!(
            "Runtime Server endpoint is not a non-writable, non-symlink current-UID file: {}",
            path.display()
        ));
    }
    if before.mode() & 0o777 == 0o600 {
        return Ok(());
    }

    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .await
        .map_err(|error| {
            format!(
                "failed to canonicalize Runtime Server endpoint permissions {}: {error}",
                path.display()
            )
        })?;
    let after = tokio::fs::symlink_metadata(path).await.map_err(|error| {
        format!(
            "failed to verify Runtime Server endpoint permissions {}: {error}",
            path.display()
        )
    })?;
    if !after.file_type().is_file()
        || after.file_type().is_symlink()
        || after.uid() != current_uid
        || after.dev() != before.dev()
        || after.ino() != before.ino()
        || after.mode() & 0o777 != 0o600
    {
        return Err(format!(
            "Runtime Server endpoint permission canonicalization lost file identity: {}",
            path.display()
        ));
    }
    Ok(())
}

fn materialize_endpoint_v1_identity(value: &mut Value) -> Result<bool, String> {
    let object = value.as_object_mut().ok_or_else(|| {
        "reasonKind=runtime-server-endpoint-identity-incomplete endpoint v1 is not an object"
            .to_owned()
    })?;
    let runtime_identity_migrated =
        canonicalize_endpoint_v1_runtime_binary_identity(object, "runtimeBinaryIdentity")?;
    let observed_identity_migrated =
        canonicalize_endpoint_v1_runtime_binary_identity(object, "observedRuntimeBinaryIdentity")?;
    let identity_migrated = runtime_identity_migrated || observed_identity_migrated;
    let missing = [
        "binaryContentDigest",
        "runtimeGenerationDigest",
        "schemaDigest",
    ]
    .iter()
    .any(|field| !object.contains_key(*field));
    if !missing {
        return Ok(identity_migrated);
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
    let binary_content_digest = runtime_identity.content_digest().to_string();
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
    let control_endpoint = object.get("controlEndpoint").cloned().ok_or_else(|| {
        "reasonKind=runtime-server-endpoint-identity-incomplete missing controlEndpoint".to_owned()
    })?;
    let control_endpoint = serde_json::to_string(&control_endpoint)
        .map_err(|error| format!("encode Runtime control endpoint identity: {error}"))?;
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
            &control_endpoint,
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

fn canonicalize_endpoint_v1_nested_content_identity(value: &mut Value) -> Result<bool, String> {
    match value {
        Value::Array(values) => {
            let mut migrated = false;
            for value in values {
                migrated |= canonicalize_endpoint_v1_nested_content_identity(value)?;
            }
            Ok(migrated)
        }
        Value::Object(object) => {
            if !object.contains_key("digest") {
                let algorithm = object.get("algorithm").and_then(Value::as_str);
                let legacy_value = object.get("value").and_then(Value::as_str);
                if let (Some("blake3-256"), Some(legacy_value)) = (algorithm, legacy_value) {
                    let digest = serde_json::from_value::<
                        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
                    >(Value::String(legacy_value.to_owned()))
                    .map_err(|error| {
                        format!(
                            "state=runtime-server-endpoint-identity-incomplete reasonKind=runtime-binary-identity-digest-invalid schemaVersion=1 error={error}"
                        )
                    })?;
                    object.clear();
                    object.insert(
                        "digest".to_owned(),
                        serde_json::to_value(digest).map_err(|error| {
                            format!(
                                "state=runtime-server-endpoint-identity-incomplete reasonKind=runtime-binary-identity-digest-encode-failed schemaVersion=1 error={error}"
                            )
                        })?,
                    );
                    return Ok(true);
                }
            }

            let mut migrated = false;
            for value in object.values_mut() {
                migrated |= canonicalize_endpoint_v1_nested_content_identity(value)?;
            }
            Ok(migrated)
        }
        _ => Ok(false),
    }
}

#[cfg(test)]
#[test]
fn endpoint_v1_nested_content_identity_is_rewritten_before_typed_deserialize() {
    let raw_digest = "a".repeat(64);
    let mut identity = serde_json::json!({
        "kind": "content",
        "identity": {
            "value": raw_digest,
            "algorithm": "blake3-256"
        }
    });

    assert!(canonicalize_endpoint_v1_nested_content_identity(&mut identity).unwrap());
    assert_eq!(
        identity.pointer("/identity/digest").and_then(Value::as_str),
        Some(format!("blake3-256:{}", "a".repeat(64)).as_str())
    );
    assert!(identity.pointer("/identity/value").is_none());
    assert!(identity.pointer("/identity/algorithm").is_none());
}

#[cfg(test)]
#[test]
fn malformed_endpoint_v1_nested_content_identity_fails_atomically() {
    let mut identity = serde_json::json!({
        "kind": "content",
        "identity": {
            "value": "not-a-blake3-digest",
            "algorithm": "blake3-256"
        }
    });
    let original = identity.clone();

    assert!(canonicalize_endpoint_v1_nested_content_identity(&mut identity).is_err());
    assert_eq!(identity, original);
}

fn canonicalize_endpoint_v1_runtime_binary_identity(
    object: &mut serde_json::Map<String, Value>,
    field: &str,
) -> Result<bool, String> {
    if let Some(identity) = object.get_mut(field)
        && canonicalize_endpoint_v1_nested_content_identity(identity)?
    {
        return Ok(true);
    }
    let identity_value = object.get(field).ok_or_else(|| {
        format!("reasonKind=runtime-server-endpoint-identity-incomplete missing {field}")
    })?;
    let identity_object = identity_value.as_object().ok_or_else(|| {
        "reasonKind=runtime-server-endpoint-identity-incomplete runtimeBinaryIdentity is not an object"
            .to_owned()
    })?;
    if identity_object.contains_key("digest") {
        return Ok(false);
    }
    if identity_object.get("kind").and_then(Value::as_str) != Some("content") {
        return Err(
            "reasonKind=runtime-server-endpoint-identity-incomplete runtimeBinaryIdentity kind is not content"
                .to_owned(),
        );
    }
    let previous_identity = identity_object
        .get("identity")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "reasonKind=runtime-server-endpoint-identity-incomplete missing authoritative runtimeBinaryIdentity identity"
                .to_owned()
        })?;
    if let Some(digest) = previous_identity.get("digest").and_then(Value::as_str) {
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(digest)
            .map_err(|error| {
                format!(
                    "reasonKind=runtime-server-endpoint-identity-incomplete owner=endpoint-v1-migration field=runtimeBinaryIdentity.identity.digest {error}"
                )
            })?;
        return Ok(false);
    }
    if previous_identity.get("algorithm").and_then(Value::as_str) != Some("blake3-256") {
        return Err(
            "reasonKind=runtime-server-endpoint-identity-incomplete runtimeBinaryIdentity algorithm is not blake3-256"
                .to_owned(),
        );
    }
    let digest = previous_identity
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "reasonKind=runtime-server-endpoint-identity-incomplete missing authoritative runtimeBinaryIdentity value"
                .to_owned()
        })?;
    let digest = agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(
        digest,
    )
    .map_err(|error| {
        format!(
            "reasonKind=runtime-server-endpoint-identity-incomplete owner=endpoint-v1-migration field=runtimeBinaryIdentity.identity.value {error}"
        )
    })?;
    let identity = RuntimeBinaryIdentity::from_content_digest(digest);
    object.insert(
        field.to_owned(),
        serde_json::to_value(identity).map_err(|error| {
            format!(
                "reasonKind=runtime-server-endpoint-identity-incomplete failed to materialize canonical runtimeBinaryIdentity: {error}"
            )
        })?,
    );
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
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .await
        .map_err(|error| {
            format!(
                "failed to stage migrated Runtime Server endpoint {}: {error}",
                temporary.display()
            )
        })?;
    file.write_all(&bytes).await.map_err(|error| {
        format!(
            "failed to write migrated Runtime Server endpoint {}: {error}",
            temporary.display()
        )
    })?;
    file.sync_all().await.map_err(|error| {
        format!(
            "failed to sync migrated Runtime Server endpoint {}: {error}",
            temporary.display()
        )
    })?;
    drop(file);
    tokio::fs::rename(&temporary, path).await.map_err(|error| {
        format!(
            "failed to atomically publish migrated Runtime Server endpoint {}: {error}",
            path.display()
        )
    })?;
    if let Some(parent) = path.parent() {
        let directory = tokio::fs::File::open(parent).await.map_err(|error| {
            format!(
                "failed to open migrated Runtime Server endpoint directory {}: {error}",
                parent.display()
            )
        })?;
        directory.sync_all().await.map_err(|error| {
            format!(
                "failed to sync migrated Runtime Server endpoint directory {}: {error}",
                parent.display()
            )
        })?;
    }
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
    let status_memory_path = PathBuf::from(&endpoint.status_memory_path);
    let owned = read_endpoint(&endpoint_path).await.is_ok_and(|actual| {
        actual.owner_epoch == endpoint.owner_epoch && actual.binding_token == endpoint.binding_token
    });
    if owned {
        let _ = tokio::fs::remove_file(endpoint_path).await;
        let _ = tokio::fs::remove_file(status_memory_path).await;
    }
    Ok(())
}

#[cfg(test)]
#[test]
fn endpoint_v1_canonical_content_identity_is_validated_without_rewrite() {
    let digest = format!("blake3-256:{}", "a".repeat(64));
    let mut endpoint = serde_json::json!({
        "runtimeBinaryIdentity": {
            "kind": "content",
            "identity": { "digest": digest }
        }
    });
    let original = endpoint.clone();
    let object = endpoint.as_object_mut().expect("endpoint object");

    assert!(
        !canonicalize_endpoint_v1_runtime_binary_identity(object, "runtimeBinaryIdentity")
            .expect("validate canonical content identity")
    );
    assert_eq!(endpoint, original);
}
