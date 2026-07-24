use agent_semantic_content_identity::{ArtifactJson, hash_normalized_json};
use serde_json::Value;

use super::SearchProjectionError;

pub const SEMANTIC_SEARCH_PACKET_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-search-packet";
pub const SEMANTIC_SEARCH_PACKET_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug)]
pub struct SemanticSearchPacketV1 {
    value: Value,
    semantic_digest: String,
}

impl SemanticSearchPacketV1 {
    pub fn from_value(value: Value) -> Result<Self, SearchProjectionError> {
        let object = value.as_object().ok_or_else(|| {
            SearchProjectionError::InvalidPacket("packet must be an object".to_string())
        })?;
        let schema_id = object.get("schemaId").and_then(Value::as_str);
        if schema_id != Some(SEMANTIC_SEARCH_PACKET_SCHEMA_ID) {
            return Err(SearchProjectionError::InvalidPacket(
                "schemaId must be agent.semantic-protocols.semantic-search-packet".to_string(),
            ));
        }
        let schema_version = object.get("schemaVersion").and_then(Value::as_str);
        if schema_version != Some(SEMANTIC_SEARCH_PACKET_SCHEMA_VERSION) {
            return Err(SearchProjectionError::InvalidPacket(
                "schemaVersion must be 1".to_string(),
            ));
        }
        for field in ["languageId", "providerId"] {
            let present = object
                .get(field)
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty());
            if !present {
                return Err(SearchProjectionError::InvalidPacket(format!(
                    "{field} must not be empty"
                )));
            }
        }

        let artifact = ArtifactJson::from_serializable(&value).map_err(|error| {
            SearchProjectionError::InvalidPacket(format!("packet canonicalization failed: {error}"))
        })?;
        let hash = hash_normalized_json(&artifact);
        let semantic_digest = format!("blake3-256:{}", hash.value);
        Ok(Self {
            value,
            semantic_digest,
        })
    }

    pub fn as_value(&self) -> &Value {
        &self.value
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }
}
