//! Provider manifest digest validation.

use sha2::{Digest, Sha256};

use crate::protocol::AgentHookError;

use super::protocol_activation_manifest::ProviderManifest;

pub fn provider_manifest_digest(manifest: &ProviderManifest) -> Result<String, AgentHookError> {
    let bytes = serde_json::to_vec(manifest).map_err(AgentHookError::InvalidOutput)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("sha256:{digest:x}"))
}

pub fn provider_execution_command_digest(command_prefix: &[String]) -> Result<String, String> {
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

    for component in command_prefix {
        update_digest_component(&mut digest, component.as_bytes());
        match std::fs::metadata(component) {
            Ok(metadata) if metadata.is_file() => {
                digest.update([1]);
                let canonical_path = std::fs::canonicalize(component).map_err(|error| {
                    format!(
                        "failed to canonicalize provider command component `{component}`: {error}"
                    )
                })?;
                update_digest_component(&mut digest, canonical_path.as_os_str().as_encoded_bytes());
                let bytes = std::fs::read(&canonical_path).map_err(|error| {
                    format!(
                        "failed to read provider command component `{}`: {error}",
                        canonical_path.display()
                    )
                })?;
                update_digest_component(&mut digest, &bytes);
            }
            Ok(_) => digest.update([0]),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => digest.update([0]),
            Err(error) => {
                return Err(format!(
                    "failed to inspect provider command component `{component}`: {error}"
                ));
            }
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
