//! Typed, framed language-projection batches for generation-time provider work.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    OutputFraming, OutputMode, ProviderProcessFraming, ProviderProcessLimits, ProviderProcessSpec,
    StdinMode, run_provider_process_async_with_framing,
};

pub const PROJECTION_BATCH_REQUEST_SCHEMA_ID: &str =
    "asp.provider-language-projection-batch-request.v1";
pub const PROJECTION_BATCH_RESPONSE_SCHEMA_ID: &str =
    "asp.provider-language-projection-batch-response.v1";
pub const PROJECTION_BATCH_TRANSPORT: &str = "framed-stdin-v1";

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProviderProjectionOwner {
    pub owner_path: String,
    pub source_leaf_digest: String,
    pub source_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProviderProjectionBatchRequest {
    pub language_id: String,
    pub provider_id: String,
    pub generation_root_digest: String,
    pub base_generation_root_digest: Option<String>,
    pub owners: Vec<ProviderProjectionOwner>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderProjectionBatchResponse {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub provider_id: String,
    pub generation_root_digest: String,
    pub owners: Vec<ProviderProjectedOwner>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderProjectedOwner {
    pub owner_path: String,
    pub source_leaf_digest: String,
    pub items: Vec<ProviderProjectedItem>,
    pub relations: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderProjectedItem {
    pub item_id: String,
    pub owner_id: String,
    pub kind: String,
    pub name: String,
    pub selector: String,
    pub source_byte_start: usize,
    pub source_byte_end: usize,
    pub identity: ProviderProjectedItemIdentity,
    pub impl_owner: Option<String>,
    pub trait_owner: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderProjectedItemIdentity {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub kind: String,
    pub symbol: String,
    pub lexical_owner: Option<String>,
    pub implementation_owner: Option<String>,
    pub trait_owner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectionBatchHeader {
    schema_id: String,
    schema_version: String,
    language_id: String,
    provider_id: String,
    transport: String,
    generation_root_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_generation_root_digest: Option<String>,
    owners: Vec<ProjectionBatchOwnerHeader>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectionBatchOwnerHeader {
    owner_path: String,
    source_leaf_digest: String,
    byte_length: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProviderProjectionBatchError(String);

impl fmt::Display for ProviderProjectionBatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ProviderProjectionBatchError {}

impl ProviderProjectionBatchRequest {
    pub fn encode(&self) -> Result<Vec<u8>, ProviderProjectionBatchError> {
        self.validate()?;
        let header = ProjectionBatchHeader {
            schema_id: PROJECTION_BATCH_REQUEST_SCHEMA_ID.to_string(),
            schema_version: "1".to_string(),
            language_id: self.language_id.clone(),
            provider_id: self.provider_id.clone(),
            transport: PROJECTION_BATCH_TRANSPORT.to_string(),
            generation_root_digest: self.generation_root_digest.clone(),
            base_generation_root_digest: self.base_generation_root_digest.clone(),
            owners: self
                .owners
                .iter()
                .map(|owner| ProjectionBatchOwnerHeader {
                    owner_path: owner.owner_path.clone(),
                    source_leaf_digest: owner.source_leaf_digest.clone(),
                    byte_length: owner.source_bytes.len(),
                })
                .collect(),
        };
        let header_bytes = serde_json::to_vec(&header).map_err(|error| {
            ProviderProjectionBatchError(format!("encode projection batch header: {error}"))
        })?;
        let header_length = u32::try_from(header_bytes.len()).map_err(|_| {
            ProviderProjectionBatchError("projection batch header exceeds u32 framing".to_string())
        })?;
        let owner_bytes = self
            .owners
            .iter()
            .map(|owner| owner.source_bytes.len())
            .sum::<usize>();
        let mut frame = Vec::with_capacity(4 + header_bytes.len() + owner_bytes);
        frame.extend_from_slice(&header_length.to_be_bytes());
        frame.extend_from_slice(&header_bytes);
        for owner in &self.owners {
            frame.extend_from_slice(&owner.source_bytes);
        }
        Ok(frame)
    }

    fn validate(&self) -> Result<(), ProviderProjectionBatchError> {
        require_text("languageId", &self.language_id)?;
        require_text("providerId", &self.provider_id)?;
        require_text("generationRootDigest", &self.generation_root_digest)?;
        if let Some(base_digest) = self.base_generation_root_digest.as_deref() {
            require_text("baseGenerationRootDigest", base_digest)?;
        }
        let mut owner_paths = BTreeSet::new();
        for owner in &self.owners {
            require_text("ownerPath", &owner.owner_path)?;
            require_text("sourceLeafDigest", &owner.source_leaf_digest)?;
            if !owner_paths.insert(owner.owner_path.as_str()) {
                return Err(ProviderProjectionBatchError(format!(
                    "duplicate projection owner path: {}",
                    owner.owner_path
                )));
            }
        }
        Ok(())
    }
}

pub async fn run_provider_projection_batch(
    program: impl Into<String>,
    command_binding: impl Into<String>,
    cwd: impl AsRef<Path>,
    request: &ProviderProjectionBatchRequest,
) -> Result<ProviderProjectionBatchResponse, ProviderProjectionBatchError> {
    let stdin = request.encode()?;
    let spec = ProviderProcessSpec {
        program: program.into(),
        args: vec![command_binding.into()],
        cwd: PathBuf::from(cwd.as_ref()),
        env: BTreeMap::new(),
        stdin: StdinMode::bytes(stdin),
        stdout: OutputMode::Capture,
        stderr: OutputMode::Capture,
        limits: ProviderProcessLimits::default(),
    };
    let output = run_provider_process_async_with_framing(
        spec,
        ProviderProcessFraming {
            stdout: OutputFraming::Bytes,
            stderr: OutputFraming::Bytes,
        },
    )
    .await
    .map_err(|error| {
        ProviderProjectionBatchError(format!("projection provider failed: {error}"))
    })?;
    if !output.status.success() {
        return Err(ProviderProjectionBatchError(format!(
            "projection provider exited with status {:?}: {}",
            output.status.code(),
            output.stderr_lossy()
        )));
    }
    let response: ProviderProjectionBatchResponse = serde_json::from_slice(&output.stdout)
        .map_err(|error| {
            ProviderProjectionBatchError(format!("decode projection batch response: {error}"))
        })?;
    validate_response(request, &response)?;
    Ok(response)
}

fn validate_response(
    request: &ProviderProjectionBatchRequest,
    response: &ProviderProjectionBatchResponse,
) -> Result<(), ProviderProjectionBatchError> {
    if response.schema_id != PROJECTION_BATCH_RESPONSE_SCHEMA_ID
        || response.schema_version != "1"
        || response.language_id != request.language_id
        || response.provider_id != request.provider_id
        || response.generation_root_digest != request.generation_root_digest
    {
        return Err(ProviderProjectionBatchError(
            "projection batch response identity mismatch".to_string(),
        ));
    }
    let expected = request
        .owners
        .iter()
        .map(|owner| (owner.owner_path.as_str(), owner.source_leaf_digest.as_str()))
        .collect::<BTreeMap<_, _>>();
    let actual = response
        .owners
        .iter()
        .map(|owner| (owner.owner_path.as_str(), owner.source_leaf_digest.as_str()))
        .collect::<BTreeMap<_, _>>();
    if expected.len() != request.owners.len()
        || actual.len() != response.owners.len()
        || actual != expected
    {
        return Err(ProviderProjectionBatchError(
            "projection batch owner coverage mismatch".to_string(),
        ));
    }
    for projected_owner in &response.owners {
        let requested_owner = request
            .owners
            .iter()
            .find(|owner| owner.owner_path == projected_owner.owner_path)
            .expect("owner coverage was checked above");
        let expected_owner_id = format!("owner:{}", projected_owner.owner_path);
        let expected_selector_prefix =
            format!("{}://{}#", request.language_id, projected_owner.owner_path);
        let mut selectors = BTreeSet::new();
        for item in &projected_owner.items {
            if item.owner_id != expected_owner_id
                || item.identity.schema_id != "asp.canonical-language-item-identity.v1"
                || item.identity.schema_version != "1"
                || item.identity.language_id != request.language_id
                || item.identity.kind != item.kind
                || item.identity.symbol != item.name
                || item.impl_owner != item.identity.implementation_owner
                || item.trait_owner != item.identity.trait_owner
                || !item.selector.starts_with(&expected_selector_prefix)
                || item.source_byte_start >= item.source_byte_end
                || item.source_byte_end > requested_owner.source_bytes.len()
                || !selectors.insert(item.selector.as_str())
            {
                return Err(ProviderProjectionBatchError(format!(
                    "projection batch item proof mismatch: ownerPath={} itemId={}",
                    projected_owner.owner_path, item.item_id
                )));
            }
        }
    }
    Ok(())
}

fn require_text(field: &str, value: &str) -> Result<(), ProviderProjectionBatchError> {
    if value.trim().is_empty() {
        return Err(ProviderProjectionBatchError(format!(
            "projection batch {field} must be non-empty"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ProviderProjectionBatchRequest {
        ProviderProjectionBatchRequest {
            language_id: "rust".to_string(),
            provider_id: "rs-harness".to_string(),
            generation_root_digest: "generation-a".to_string(),
            base_generation_root_digest: Some("generation-base".to_string()),
            owners: vec![ProviderProjectionOwner {
                owner_path: "src/lib.rs".to_string(),
                source_leaf_digest: "leaf-a".to_string(),
                source_bytes: b"pub fn exact() {}\n".to_vec(),
            }],
        }
    }

    #[test]
    fn framed_request_carries_exact_owner_bytes() {
        let request = request();
        let frame = request.encode().expect("encode request");
        let header_length = u32::from_be_bytes(frame[..4].try_into().expect("header length"));
        let header_end = 4 + header_length as usize;
        let header: ProjectionBatchHeader =
            serde_json::from_slice(&frame[4..header_end]).expect("decode header");
        assert_eq!(header.transport, PROJECTION_BATCH_TRANSPORT);
        assert_eq!(
            header.owners[0].byte_length,
            request.owners[0].source_bytes.len()
        );
        assert_eq!(&frame[header_end..], request.owners[0].source_bytes);
    }

    #[test]
    fn response_validation_rejects_generation_or_owner_drift() {
        let request = request();
        let mut response = ProviderProjectionBatchResponse {
            schema_id: PROJECTION_BATCH_RESPONSE_SCHEMA_ID.to_string(),
            schema_version: "1".to_string(),
            language_id: "rust".to_string(),
            provider_id: "rs-harness".to_string(),
            generation_root_digest: "generation-a".to_string(),
            owners: vec![ProviderProjectedOwner {
                owner_path: "src/lib.rs".to_string(),
                source_leaf_digest: "leaf-a".to_string(),
                items: vec![ProviderProjectedItem {
                    item_id: "item:function:exact".to_string(),
                    owner_id: "owner:src/lib.rs".to_string(),
                    kind: "function".to_string(),
                    name: "exact".to_string(),
                    selector: "rust://src/lib.rs#item/function/exact".to_string(),
                    source_byte_start: 0,
                    source_byte_end: request.owners[0].source_bytes.len(),
                    identity: ProviderProjectedItemIdentity {
                        schema_id: "asp.canonical-language-item-identity.v1".to_string(),
                        schema_version: "1".to_string(),
                        language_id: "rust".to_string(),
                        kind: "function".to_string(),
                        symbol: "exact".to_string(),
                        lexical_owner: None,
                        implementation_owner: None,
                        trait_owner: None,
                    },
                    impl_owner: None,
                    trait_owner: None,
                }],
                relations: Vec::new(),
            }],
        };
        validate_response(&request, &response).expect("matching response");
        response.owners[0].source_leaf_digest = "leaf-drift".to_string();
        assert!(validate_response(&request, &response).is_err());
    }
}
