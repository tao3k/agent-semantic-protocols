// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest;
use serde::Deserialize;
use serde::Serialize;

pub const RUNTIME_SERVER_GENERATION_MISMATCH: &str = "runtime-server-generation-mismatch";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeServerGenerationIdentity {
    pub binary_content_digest: String,
    pub runtime_generation_digest: String,
    pub schema_digest: String,
}

impl RuntimeServerGenerationIdentity {
    pub fn derive(
        binary_content_digest: impl Into<String>,
        schema_id: &str,
        schema_version: &str,
        transport_contract_digest: &str,
        artifact_catalog_digest: &str,
        owner_epoch: u64,
    ) -> Self {
        let binary_content_digest = binary_content_digest.into();
        let schema_material = format!("{schema_id}\0{schema_version}\0{transport_contract_digest}");
        let schema_digest = Blake3ContentDigest::from_bytes(schema_material.as_bytes()).to_string();
        let runtime_generation_material = format!(
            "{binary_content_digest}\0{schema_digest}\0{artifact_catalog_digest}\0{owner_epoch}"
        );
        let runtime_generation_digest =
            Blake3ContentDigest::from_bytes(runtime_generation_material.as_bytes()).to_string();
        Self {
            binary_content_digest,
            runtime_generation_digest,
            schema_digest,
        }
    }

    pub fn validate(&self, observed: &Self) -> Result<(), RuntimeServerGenerationMismatch> {
        if self == observed {
            return Ok(());
        }
        Err(RuntimeServerGenerationMismatch {
            reason_kind: RUNTIME_SERVER_GENERATION_MISMATCH,
            expected: self.clone(),
            observed: observed.clone(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerGenerationMismatch {
    pub reason_kind: &'static str,
    pub expected: RuntimeServerGenerationIdentity,
    pub observed: RuntimeServerGenerationIdentity,
}

impl std::fmt::Display for RuntimeServerGenerationMismatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let payload = serde_json::to_string(self).map_err(|_| std::fmt::Error)?;
        formatter.write_str(&payload)
    }
}

impl std::error::Error for RuntimeServerGenerationMismatch {}

#[cfg(test)]
#[path = "../tests/unit/runtime_generation.rs"]
mod tests;
