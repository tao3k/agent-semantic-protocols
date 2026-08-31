use serde::{Deserialize, Serialize};

use crate::content_binding::{ContentBinding, ContentBindingError};

pub const RUNTIME_EXECUTION_SCHEMA_ID: &str = "asp.runtime-execution-binding";
pub const RUNTIME_EXECUTION_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeExecutionBinding {
    pub schema_id: String,
    pub schema_version: String,
    pub content_binding: ContentBinding,
    pub runtime_artifact_digest: String,
    pub evaluator_policy_digest: String,
    pub active_artifact_receipt_digest: String,
    pub evaluator_abi_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeExecutionBindingError {
    Binding(ContentBindingError),
    SchemaMismatch,
    InvalidDigest { field: &'static str },
    ContentMismatch,
}

impl RuntimeExecutionBinding {
    pub fn new(
        content_binding: ContentBinding,
        runtime_artifact_digest: impl Into<String>,
        evaluator_policy_digest: impl Into<String>,
        active_artifact_receipt_digest: impl Into<String>,
        evaluator_abi_digest: impl Into<String>,
    ) -> Result<Self, RuntimeExecutionBindingError> {
        let binding = Self {
            schema_id: RUNTIME_EXECUTION_SCHEMA_ID.into(),
            schema_version: RUNTIME_EXECUTION_SCHEMA_VERSION.into(),
            content_binding,
            runtime_artifact_digest: runtime_artifact_digest.into(),
            evaluator_policy_digest: evaluator_policy_digest.into(),
            active_artifact_receipt_digest: active_artifact_receipt_digest.into(),
            evaluator_abi_digest: evaluator_abi_digest.into(),
        };
        binding.validate()?;
        Ok(binding)
    }

    pub fn validate(&self) -> Result<(), RuntimeExecutionBindingError> {
        if self.schema_id != RUNTIME_EXECUTION_SCHEMA_ID
            || self.schema_version != RUNTIME_EXECUTION_SCHEMA_VERSION
        {
            return Err(RuntimeExecutionBindingError::SchemaMismatch);
        }
        self.content_binding
            .validate()
            .map_err(RuntimeExecutionBindingError::Binding)?;
        for (field, value) in [
            ("runtimeArtifactDigest", &self.runtime_artifact_digest),
            ("evaluatorPolicyDigest", &self.evaluator_policy_digest),
            (
                "activeArtifactReceiptDigest",
                &self.active_artifact_receipt_digest,
            ),
            ("evaluatorAbiDigest", &self.evaluator_abi_digest),
        ] {
            if !is_content_digest(value) {
                return Err(RuntimeExecutionBindingError::InvalidDigest { field });
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        for value in [
            self.content_binding.identity.digest(),
            self.runtime_artifact_digest.clone(),
            self.evaluator_policy_digest.clone(),
            self.active_artifact_receipt_digest.clone(),
            self.evaluator_abi_digest.clone(),
        ] {
            hasher.update(value.as_bytes());
            hasher.update(&[0]);
        }
        format!("blake3-256:{}", hasher.finalize().to_hex())
    }

    pub fn admits(&self, other: &Self) -> Result<(), RuntimeExecutionBindingError> {
        self.validate()?;
        other.validate()?;
        if self != other {
            return Err(RuntimeExecutionBindingError::ContentMismatch);
        }
        Ok(())
    }
}

fn is_content_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("blake3-256:") else {
        return false;
    };
    hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
