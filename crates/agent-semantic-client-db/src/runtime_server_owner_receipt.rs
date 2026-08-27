//! Runtime Server owner receipt, owned by client-db lifecycle composition.
use serde::{Deserialize, Serialize};

pub const RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-owner-spawn.v1";
pub const RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_VERSION: &str = "1";
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerSpawnReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub process_id: u32,
    pub nonce: String,
    pub state_home: String,
    pub activation_generation: u64,
    pub launcher_artifact_path: String,
    pub launcher_artifact_digest:
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    pub spawn_argv: Vec<String>,
    pub previous_serving_digest:
        Option<agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest>,
    #[serde(default)]
    pub previous_owner_epoch: Option<u64>,
}

/// The original stable-v1 observation shape. It is intentionally decoded only
/// far enough to distinguish a well-formed legacy observation from corrupted
/// state. It never becomes current launcher authority.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyRuntimeServerSpawnReceiptV1 {
    schema_id: String,
    schema_version: String,
    process_id: u32,
    nonce: String,
    state_home: String,
    runtime_artifact_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaleRuntimeServerSpawnReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub reason_kind: String,
}

#[derive(Clone, Debug)]
pub enum RuntimeServerSpawnReceiptRead {
    Current(RuntimeServerSpawnReceipt),
    Stale(StaleRuntimeServerSpawnReceipt),
}

pub fn decode_runtime_server_spawn_receipt(
    bytes: &[u8],
) -> Result<RuntimeServerSpawnReceiptRead, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|error| {
        owner_spawn_decode_error("runtime-server-owner-spawn-malformed", error.to_string())
    })?;
    let object = value.as_object().ok_or_else(|| {
        owner_spawn_decode_error(
            "runtime-server-owner-spawn-malformed",
            "owner-spawn receipt must be a JSON object",
        )
    })?;
    let schema_id = object
        .get("schemaId")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            owner_spawn_decode_error(
                "runtime-server-owner-spawn-malformed",
                "owner-spawn receipt requires schemaId",
            )
        })?;
    let schema_version = object
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            owner_spawn_decode_error(
                "runtime-server-owner-spawn-malformed",
                "owner-spawn receipt requires schemaVersion",
            )
        })?;
    if schema_id != RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID
        || schema_version != RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_VERSION
    {
        return Err(owner_spawn_decode_error(
            "runtime-server-owner-spawn-unknown-schema",
            format!("unsupported owner-spawn schema {schema_id}@{schema_version}"),
        ));
    }

    const LAUNCHER_AUTHORITY_FIELDS: [&str; 5] = [
        "activationGeneration",
        "launcherArtifactPath",
        "launcherArtifactDigest",
        "spawnArgv",
        "previousServingDigest",
    ];
    let present = LAUNCHER_AUTHORITY_FIELDS
        .iter()
        .filter(|field| object.contains_key(**field))
        .count();
    if present == 0 {
        let legacy: LegacyRuntimeServerSpawnReceiptV1 = serde_json::from_value(value.clone())
            .map_err(|error| {
                owner_spawn_decode_error(
                    "runtime-server-owner-spawn-malformed-legacy-v1",
                    error.to_string(),
                )
            })?;
        if legacy.schema_id != RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID
            || legacy.schema_version != RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_VERSION
            || legacy.process_id == 0
            || legacy.nonce.is_empty()
            || !std::path::Path::new(&legacy.state_home).is_absolute()
            || !std::path::Path::new(&legacy.runtime_artifact_path).is_absolute()
        {
            return Err(owner_spawn_decode_error(
                "runtime-server-owner-spawn-invalid-legacy-v1",
                "legacy v1 owner-spawn observation is incomplete or non-canonical",
            ));
        }
        return Ok(RuntimeServerSpawnReceiptRead::Stale(
            StaleRuntimeServerSpawnReceipt {
                schema_id: schema_id.to_owned(),
                schema_version: schema_version.to_owned(),
                reason_kind: "runtime-server-owner-spawn-launcher-authority-stale".to_owned(),
            },
        ));
    }
    if present != LAUNCHER_AUTHORITY_FIELDS.len() {
        return Err(owner_spawn_decode_error(
            "runtime-server-owner-spawn-partial-launcher-authority",
            "owner-spawn receipt has a partial launcher authority",
        ));
    }

    let receipt: RuntimeServerSpawnReceipt = serde_json::from_value(value).map_err(|error| {
        owner_spawn_decode_error("runtime-server-owner-spawn-malformed", error.to_string())
    })?;
    if receipt.activation_generation == 0 {
        return Err(owner_spawn_decode_error(
            "runtime-server-owner-spawn-invalid-launcher-authority",
            "activationGeneration must be positive",
        ));
    }
    if !std::path::Path::new(&receipt.launcher_artifact_path).is_absolute()
        || receipt.spawn_argv.is_empty()
    {
        return Err(owner_spawn_decode_error(
            "runtime-server-owner-spawn-invalid-launcher-authority",
            "launcherArtifactPath must be absolute and spawnArgv must be non-empty",
        ));
    }
    Ok(RuntimeServerSpawnReceiptRead::Current(receipt))
}

fn owner_spawn_decode_error(reason_kind: &str, message: impl Into<String>) -> String {
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.runtime-server-owner-spawn-read-receipt",
        "schemaVersion": "1",
        "state": "failed",
        "reasonKind": reason_kind,
        "message": message.into(),
    })
    .to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerExitReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub owner_epoch: u64,
    pub clean_drain: bool,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerDrainReceipt {
    pub owner_epoch: u64,
    pub services: serde_json::Value,
    pub remaining_task_count: usize,
    pub remaining_child_count: usize,
    pub clean_drain: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(serde::Deserialize)]
pub struct RuntimeServerResidentTransactionReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub state: String,
    pub activation_generation: u64,
    pub launcher_artifact_path: String,
    pub launcher_artifact_digest:
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    pub spawn_argv: Vec<String>,
    pub applied_artifact_digest:
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    pub applied_activation_generation: u64,
    pub endpoint_owner_epoch: u64,
    pub endpoint_binary_content_digest:
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    pub endpoint_runtime_generation_digest: String,
    pub control_endpoint: String,
    pub data_endpoint: String,
    pub provider_endpoint: String,
    pub previous_serving_digest:
        Option<agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest>,
    pub previous_owner_epoch: Option<u64>,
    pub previous_drain_state: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeServerActivationReadyReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub state: String,
    pub activation_generation: u64,
    pub artifact_digest: agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    pub owner_epoch: u64,
    pub launcher_receipt_digest:
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
}
