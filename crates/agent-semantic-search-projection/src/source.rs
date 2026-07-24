use agent_semantic_content_identity::{ArtifactJson, hash_normalized_json};
use serde_json::Value;

use crate::{SearchProjectionError, SemanticSearchPacketV1};

pub const SEMANTIC_GRAPH_TURBO_RESULT_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-graph-turbo-result";
pub const SEMANTIC_GRAPH_TURBO_RESULT_SCHEMA_VERSION: &str = "1";

pub trait SearchProjectionSource {
    fn as_value(&self) -> &Value;
    fn semantic_digest(&self) -> &str;
}

impl SearchProjectionSource for SemanticSearchPacketV1 {
    fn as_value(&self) -> &Value {
        SemanticSearchPacketV1::as_value(self)
    }

    fn semantic_digest(&self) -> &str {
        SemanticSearchPacketV1::semantic_digest(self)
    }
}

#[derive(Clone, Debug)]
pub struct GraphTurboResultPacketV1 {
    value: Value,
    semantic_digest: String,
}

impl GraphTurboResultPacketV1 {
    pub fn from_value(value: Value) -> Result<Self, SearchProjectionError> {
        let object = value.as_object().ok_or_else(|| {
            SearchProjectionError::InvalidPacket("graph-turbo result must be an object".to_string())
        })?;
        require_exact_string(object, "schemaId", SEMANTIC_GRAPH_TURBO_RESULT_SCHEMA_ID)?;
        require_exact_string(
            object,
            "schemaVersion",
            SEMANTIC_GRAPH_TURBO_RESULT_SCHEMA_VERSION,
        )?;
        require_exact_string(
            object,
            "protocolId",
            "agent.semantic-protocols.semantic-language",
        )?;
        require_exact_string(object, "protocolVersion", "1")?;
        require_exact_string(object, "packetKind", "graph-turbo-result")?;
        require_non_empty_string(object, "profile")?;
        require_non_empty_string(object, "algorithm")?;
        require_string_array(object, "seedIds")?;
        require_object_array(object, "rankedNodes")?;
        require_object_array(object, "edges")?;

        let artifact = ArtifactJson::from_serializable(&value).map_err(|error| {
            SearchProjectionError::InvalidPacket(format!(
                "graph-turbo result canonicalization failed: {error}"
            ))
        })?;
        let hash = hash_normalized_json(&artifact);
        Ok(Self {
            value,
            semantic_digest: format!("blake3-256:{}", hash.value),
        })
    }
}

impl SearchProjectionSource for GraphTurboResultPacketV1 {
    fn as_value(&self) -> &Value {
        &self.value
    }

    fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }
}

fn require_exact_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), SearchProjectionError> {
    if object.get(field).and_then(Value::as_str) == Some(expected) {
        return Ok(());
    }
    Err(SearchProjectionError::InvalidPacket(format!(
        "{field} must be {expected}"
    )))
}

fn require_non_empty_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), SearchProjectionError> {
    if object
        .get(field)
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Ok(());
    }
    Err(SearchProjectionError::InvalidPacket(format!(
        "{field} must not be empty"
    )))
}

fn require_string_array(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), SearchProjectionError> {
    if object
        .get(field)
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .all(|item| item.as_str().is_some_and(|value| !value.is_empty()))
        })
    {
        return Ok(());
    }
    Err(SearchProjectionError::InvalidPacket(format!(
        "{field} must be an array of non-empty strings"
    )))
}

fn require_object_array(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), SearchProjectionError> {
    if object
        .get(field)
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().all(Value::is_object))
    {
        return Ok(());
    }
    Err(SearchProjectionError::InvalidPacket(format!(
        "{field} must be an array of objects"
    )))
}
