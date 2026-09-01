use agent_semantic_content_identity::{ArtifactJson, hash_normalized_json};
use serde_json::Value;

use crate::{SearchProjectionError, SemanticSearchPacketV1};

pub const SEMANTIC_GRAPH_TURBO_RESULT_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-graph-turbo-result";
pub const SEMANTIC_GRAPH_TURBO_RESULT_SCHEMA_VERSION: &str = "1";
pub const SEMANTIC_GRAPH_TURBO_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-graph-turbo-request";

#[derive(Clone, Debug)]
pub struct GraphTurboEvaluationRequest {
    value: Value,
}

impl GraphTurboEvaluationRequest {
    pub fn from_value(value: Value) -> Result<Self, SearchProjectionError> {
        let object = value.as_object().ok_or_else(|| {
            SearchProjectionError::InvalidPacket("graph-turbo request must be an object".to_owned())
        })?;
        reject_unknown_request_fields(object)?;
        require_exact_string(object, "schemaId", SEMANTIC_GRAPH_TURBO_REQUEST_SCHEMA_ID)?;
        require_exact_string(object, "schemaVersion", "1")?;
        require_exact_string(
            object,
            "protocolId",
            "agent.semantic-protocols.semantic-language",
        )?;
        require_exact_string(object, "protocolVersion", "1")?;
        require_exact_string(object, "packetKind", "graph-turbo-request")?;
        require_allowed_string(
            object,
            "surface",
            &[
                "search-pipe",
                "search-rg",
                "search-fd",
                "search-lexical",
                "search-ingest",
                "evidence-analyze",
            ],
        )?;
        require_object(object, "sourceSnapshot")?;
        require_object(object, "workspaceGeneration")?;
        require_unique_string_array(object, "queryTerms")?;
        require_non_empty_string(object, "profile")?;
        require_exact_string(object, "algorithm", "typed-ppr-diverse")?;
        require_unique_string_array(object, "seedIds")?;
        require_positive_integer(object, "budget")?;
        validate_optional_request_fields(object)?;
        Ok(Self { value })
    }

    pub fn into_value(self) -> Value {
        self.value
    }
}

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

    pub fn into_value(self) -> Value {
        self.value
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

fn require_allowed_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
    allowed: &[&str],
) -> Result<(), SearchProjectionError> {
    if object
        .get(field)
        .and_then(Value::as_str)
        .is_some_and(|value| allowed.contains(&value))
    {
        return Ok(());
    }
    Err(SearchProjectionError::InvalidPacket(format!(
        "{field} is not an admitted v1 value"
    )))
}

fn require_object(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), SearchProjectionError> {
    if object.get(field).is_some_and(Value::is_object) {
        return Ok(());
    }
    Err(SearchProjectionError::InvalidPacket(format!(
        "{field} must be an object"
    )))
}

fn require_positive_integer(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), SearchProjectionError> {
    if object
        .get(field)
        .and_then(Value::as_u64)
        .is_some_and(|value| value > 0)
    {
        return Ok(());
    }
    Err(SearchProjectionError::InvalidPacket(format!(
        "{field} must be a positive integer"
    )))
}

fn require_unique_string_array(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), SearchProjectionError> {
    let items = object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| SearchProjectionError::InvalidPacket(format!("{field} must be an array")))?;
    let mut unique = std::collections::HashSet::with_capacity(items.len());
    if items.iter().all(|item| {
        item.as_str()
            .is_some_and(|value| !value.is_empty() && unique.insert(value))
    }) {
        return Ok(());
    }
    Err(SearchProjectionError::InvalidPacket(format!(
        "{field} must contain unique non-empty strings"
    )))
}

fn reject_unknown_request_fields(
    object: &serde_json::Map<String, Value>,
) -> Result<(), SearchProjectionError> {
    const FIELDS: &[&str] = &[
        "schemaId",
        "schemaVersion",
        "protocolId",
        "protocolVersion",
        "packetKind",
        "surface",
        "queryTerms",
        "queryClauses",
        "queryAdjustmentPolicy",
        "profile",
        "algorithm",
        "surfaces",
        "source",
        "sourceSnapshot",
        "workspaceGeneration",
        "candidateSources",
        "sourceTrace",
        "seedIds",
        "seedPlan",
        "budget",
        "kindBudgets",
        "windowMerge",
        "pathBudget",
        "pathMaxHops",
        "cache",
        "readMemory",
        "delegationHints",
        "actionFrontier",
        "route",
        "requestId",
        "producer",
        "project",
        "summary",
        "fields",
        "graph",
        "graphs",
    ];
    if let Some(field) = object
        .keys()
        .find(|field| !FIELDS.contains(&field.as_str()))
    {
        return Err(SearchProjectionError::InvalidPacket(format!(
            "unknown graph-turbo request field: {field}"
        )));
    }
    Ok(())
}

fn validate_optional_request_fields(
    object: &serde_json::Map<String, Value>,
) -> Result<(), SearchProjectionError> {
    for field in [
        "queryClauses",
        "surfaces",
        "candidateSources",
        "sourceTrace",
        "delegationHints",
        "actionFrontier",
        "graphs",
    ] {
        if object.get(field).is_some_and(|value| !value.is_array()) {
            return Err(SearchProjectionError::InvalidPacket(format!(
                "{field} must be an array"
            )));
        }
    }
    for field in [
        "queryAdjustmentPolicy",
        "seedPlan",
        "kindBudgets",
        "windowMerge",
        "cache",
        "readMemory",
        "producer",
        "project",
        "summary",
        "fields",
        "graph",
    ] {
        if object.get(field).is_some_and(|value| !value.is_object()) {
            return Err(SearchProjectionError::InvalidPacket(format!(
                "{field} must be an object"
            )));
        }
    }
    for field in ["pathBudget", "pathMaxHops"] {
        if object.contains_key(field) {
            require_positive_integer(object, field)?;
        }
    }
    if object.contains_key("source") {
        require_allowed_string(
            object,
            "source",
            &["auto", "provider", "finder", "search-overlay", "ingest"],
        )?;
    }
    if object
        .get("requestId")
        .is_some_and(|value| !value.is_string())
    {
        return Err(SearchProjectionError::InvalidPacket(
            "requestId must be a string".to_owned(),
        ));
    }
    Ok(())
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
