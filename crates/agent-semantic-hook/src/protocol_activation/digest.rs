// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Provider manifest digest validation.

use sha2::Digest;
use sha2::Sha256;

use crate::protocol::AgentHookError;

use super::protocol_activation_manifest::ProviderManifest;

pub fn provider_manifest_digest(manifest: &ProviderManifest) -> Result<String, AgentHookError> {
    let bytes = serde_json::to_vec(manifest).map_err(AgentHookError::InvalidOutput)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("sha256:{digest:x}"))
}

pub fn provider_execution_command_digest(
    command_prefix: &[String],
    verified_executable_artifact_digest: &str,
) -> Result<String, String> {
    let executable = command_prefix
        .first()
        .ok_or_else(|| "provider execution command prefix must not be empty".to_string())?;
    let executable_metadata = std::fs::metadata(executable).map_err(|error| {
        format!("failed to resolve provider executable `{executable}` for digest: {error}")
    })?;
    if !executable_metadata.is_file() {
        return Err(format!(
            "provider executable `{executable}` is not a regular file"
        ));
    }

    let mut digest = Sha256::new();
    update_digest_component(&mut digest, b"asp-provider-execution-v1");
    digest.update((command_prefix.len() as u64).to_be_bytes());

    for (index, component) in command_prefix.iter().enumerate() {
        update_digest_component(&mut digest, component.as_bytes());
        if index == 0 {
            let canonical_path = std::fs::canonicalize(component).map_err(|error| {
                format!("failed to canonicalize provider command component `{component}`: {error}")
            })?;
            update_digest_component(&mut digest, canonical_path.as_os_str().as_encoded_bytes());
            update_digest_component(&mut digest, verified_executable_artifact_digest.as_bytes());
        }
    }

    let digest = digest.finalize();
    let digest = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!("sha256:{digest}"))
}

fn update_digest_component(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}
