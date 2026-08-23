use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

const SCHEMA_ID: &str = "agent.semantic-protocols.provider-runtime-contract-receipt";
const SCHEMA_VERSION: &str = "1";
const DIGEST_DOMAIN: &[u8] = b"asp.provider-runtime-contract-receipt.v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRuntimeContractTransport {
    RuntimeIpc,
    HttpJson,
    InProcess,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRuntimeContractOperation {
    pub operation: String,
    pub request_schema_id: String,
    pub response_schema_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRuntimeContractReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub provider_id: String,
    pub language_id: String,
    pub artifact_digest: String,
    pub registration_digest: String,
    pub contract_digest: String,
    pub transport: ProviderRuntimeContractTransport,
    pub operations: Vec<ProviderRuntimeContractOperation>,
}

impl ProviderRuntimeContractReceipt {
    pub fn new(
        provider_id: impl Into<String>,
        language_id: impl Into<String>,
        artifact_digest: impl Into<String>,
        registration_digest: impl Into<String>,
        transport: ProviderRuntimeContractTransport,
        operations: Vec<ProviderRuntimeContractOperation>,
    ) -> Result<Self, String> {
        let mut receipt = Self {
            schema_id: SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            provider_id: provider_id.into(),
            language_id: language_id.into(),
            artifact_digest: artifact_digest.into(),
            registration_digest: registration_digest.into(),
            contract_digest: String::new(),
            transport,
            operations,
        };
        receipt.contract_digest = receipt.expected_contract_digest()?;
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != SCHEMA_ID || self.schema_version != SCHEMA_VERSION {
            return Err("provider runtime contract receipt schema is unsupported".to_owned());
        }
        for (field, value) in [
            ("providerId", &self.provider_id),
            ("languageId", &self.language_id),
            ("artifactDigest", &self.artifact_digest),
            ("registrationDigest", &self.registration_digest),
            ("contractDigest", &self.contract_digest),
        ] {
            if value.trim().is_empty() {
                return Err(format!(
                    "provider runtime contract receipt {field} is required"
                ));
            }
        }
        for (field, digest) in [
            ("artifactDigest", &self.artifact_digest),
            ("registrationDigest", &self.registration_digest),
            ("contractDigest", &self.contract_digest),
        ] {
            validate_digest(field, digest)?;
        }
        if self.operations.is_empty() {
            return Err("provider runtime contract receipt operations are required".to_owned());
        }
        let mut operations = BTreeSet::new();
        for operation in &self.operations {
            if operation.operation.trim().is_empty()
                || operation.request_schema_id.trim().is_empty()
                || operation.response_schema_id.trim().is_empty()
            {
                return Err("provider runtime contract operation fields are required".to_owned());
            }
            if !operations.insert(operation.operation.as_str()) {
                return Err(format!(
                    "provider runtime contract operation is duplicated: {}",
                    operation.operation
                ));
            }
        }
        let expected_contract_digest = self.expected_contract_digest()?;
        if self.contract_digest != expected_contract_digest {
            return Err(format!(
                "provider runtime contract receipt digest mismatch: expected={expected_contract_digest} actual={} providerId={} languageId={} artifactDigest={} registrationDigest={} transport={:?} operations={}",
                self.contract_digest,
                self.provider_id,
                self.language_id,
                self.artifact_digest,
                self.registration_digest,
                self.transport,
                self.operations
                    .iter()
                    .map(|operation| operation.operation.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        Ok(())
    }

    pub fn expected_contract_digest(&self) -> Result<String, String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Contract<'a> {
            schema_id: &'a str,
            schema_version: &'a str,
            provider_id: &'a str,
            language_id: &'a str,
            artifact_digest: &'a str,
            registration_digest: &'a str,
            transport: &'a ProviderRuntimeContractTransport,
            operations: &'a [ProviderRuntimeContractOperation],
        }
        let encoded = serde_json::to_vec(&Contract {
            schema_id: &self.schema_id,
            schema_version: &self.schema_version,
            provider_id: &self.provider_id,
            language_id: &self.language_id,
            artifact_digest: &self.artifact_digest,
            registration_digest: &self.registration_digest,
            transport: &self.transport,
            operations: &self.operations,
        })
        .map_err(|error| format!("encode provider runtime contract receipt: {error}"))?;
        Ok(format!(
            "blake3-256:{}",
            agent_semantic_content_identity::exact_selector_merkle::canonical_content_digest(
                DIGEST_DOMAIN,
                &[&encoded],
            )
            .as_str()
        ))
    }
}

fn validate_digest(field: &str, digest: &str) -> Result<(), String> {
    let Some(hex) = digest
        .strip_prefix("blake3-256:")
        .or_else(|| digest.strip_prefix("sha256:"))
    else {
        return Err(format!(
            "provider runtime contract receipt {field} must use blake3-256 or sha256"
        ));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return Err(format!(
            "provider runtime contract receipt {field} must contain 64 lowercase hex characters"
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/runtime_contract.rs"]
mod tests;
