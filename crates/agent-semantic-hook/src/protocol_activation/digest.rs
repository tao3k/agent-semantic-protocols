//! Provider manifest digest validation.

use sha2::{Digest, Sha256};

use crate::protocol::AgentHookError;

use super::protocol_activation_manifest::ProviderManifest;

pub fn provider_manifest_digest(manifest: &ProviderManifest) -> Result<String, AgentHookError> {
    let bytes = serde_json::to_vec(manifest).map_err(AgentHookError::InvalidOutput)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("sha256:{digest:x}"))
}
