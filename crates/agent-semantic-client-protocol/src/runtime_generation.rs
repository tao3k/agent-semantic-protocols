use serde::{Deserialize, Serialize};

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
        let schema_digest = blake3::hash(
            format!("{schema_id}\0{schema_version}\0{transport_contract_digest}").as_bytes(),
        )
        .to_hex()
        .to_string();
        let runtime_generation_digest = blake3::hash(
            format!(
                "{binary_content_digest}\0{schema_digest}\0{artifact_catalog_digest}\0{owner_epoch}"
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string();
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
mod tests {
    use super::*;

    fn identity(binary: &str, owner_epoch: u64) -> RuntimeServerGenerationIdentity {
        RuntimeServerGenerationIdentity::derive(
            binary,
            "agent.semantic-protocols.runtime-server-endpoint",
            "1",
            "transport",
            "catalog",
            owner_epoch,
        )
    }

    #[test]
    fn new_client_cannot_bind_an_old_server_generation() {
        let error = identity("new", 2)
            .validate(&identity("old", 1))
            .unwrap_err();
        assert_eq!(error.reason_kind, RUNTIME_SERVER_GENERATION_MISMATCH);
    }

    #[test]
    fn exact_generation_identity_is_admitted() {
        let expected = identity("same", 7);
        expected.validate(&expected).unwrap();
    }
}
