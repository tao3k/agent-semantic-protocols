use agent_semantic_content_identity::{ArtifactJson, hash_normalized_json};
use serde_json::Value;

use crate::{SearchProjectionError, SemanticSearchPacketV1};

pub const SEMANTIC_GRAPH_TURBO_RESULT_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-graph-turbo-result";
pub const SEMANTIC_GRAPH_TURBO_RESULT_SCHEMA_VERSION: &str = "1";
pub const SEMANTIC_GRAPH_RESIDENT_EVALUATION_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-graph-resident-evaluation-request";
pub const SEMANTIC_GRAPH_RESIDENT_EVALUATION_RESULT_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-graph-resident-evaluation-result";

#[derive(Clone, Debug)]
pub struct ResidentGraphEvaluationRequestV1 {
    value: Value,
}

impl ResidentGraphEvaluationRequestV1 {
    pub fn from_value(value: Value) -> Result<Self, SearchProjectionError> {
        let object = value.as_object().ok_or_else(|| {
            SearchProjectionError::InvalidPacket(
                "resident graph evaluation request must be an object".to_owned(),
            )
        })?;
        reject_unknown_resident_evaluation_fields(object)?;
        require_exact_string(
            object,
            "schemaId",
            SEMANTIC_GRAPH_RESIDENT_EVALUATION_REQUEST_SCHEMA_ID,
        )?;
        require_exact_string(object, "schemaVersion", "1")?;
        require_exact_string(object, "protocolId", "agent.semantic-protocols.search")?;
        require_exact_string(object, "protocolVersion", "1")?;
        require_exact_string(object, "packetKind", "resident-graph-evaluation-request")?;
        require_non_empty_string(object, "languageId")?;
        let language_id = object["languageId"].as_str().expect("validated languageId");
        if !is_language_identifier(language_id) {
            return Err(SearchProjectionError::InvalidPacket(
                "languageId must be a lowercase hyphenated identifier".to_owned(),
            ));
        }
        require_allowed_string(
            object,
            "surface",
            &[
                "search-pipe",
                "search-rg",
                "search-lexical",
                "search-owner",
                "query",
            ],
        )?;
        require_bounded_unique_string_array(object, "queryTerms", 32, 256)?;
        require_allowed_string(object, "profile", &["balanced", "structural", "dependency"])?;
        require_bounded_unique_string_array(object, "entryNodeIds", 128, 1024)?;
        let query_terms = object["queryTerms"].as_array().expect("validated array");
        let entry_node_ids = object["entryNodeIds"].as_array().expect("validated array");
        if query_terms.is_empty() && entry_node_ids.is_empty() {
            return Err(SearchProjectionError::InvalidPacket(
                "resident graph evaluation requires queryTerms or entryNodeIds".to_owned(),
            ));
        }
        validate_resident_evaluation_budget(object)?;
        Ok(Self { value })
    }

    #[must_use]
    pub fn as_value(&self) -> &Value {
        &self.value
    }

    pub fn into_value(self) -> Value {
        self.value
    }
}

#[derive(Clone, Debug)]
pub struct ResidentGraphEvaluationResultV1 {
    value: Value,
    semantic_digest: String,
}

impl ResidentGraphEvaluationResultV1 {
    pub fn from_value(value: Value) -> Result<Self, SearchProjectionError> {
        let object = value.as_object().ok_or_else(|| {
            SearchProjectionError::InvalidPacket(
                "resident graph evaluation result must be an object".to_owned(),
            )
        })?;
        require_exact_string(
            object,
            "schemaId",
            SEMANTIC_GRAPH_RESIDENT_EVALUATION_RESULT_SCHEMA_ID,
        )?;
        require_exact_string(object, "schemaVersion", "1")?;
        require_exact_string(object, "protocolId", "agent.semantic-protocols.search")?;
        require_exact_string(object, "protocolVersion", "1")?;
        require_exact_string(object, "packetKind", "resident-graph-evaluation-result")?;
        require_exact_string(object, "state", "Ready")?;
        require_non_empty_string(object, "workspaceIdentity")?;
        require_non_empty_string(object, "generationDigest")?;
        require_non_empty_string(object, "rootDigest")?;
        require_object_array(object, "rankedNodes")?;
        require_object_array(object, "edges")?;
        let work = object
            .get("workCounters")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                SearchProjectionError::InvalidPacket("workCounters must be an object".to_owned())
            })?;
        for field in [
            "providerRpcCount",
            "durableReadCount",
            "generationMutationCount",
        ] {
            if work.get(field).and_then(Value::as_u64) != Some(0) {
                return Err(SearchProjectionError::InvalidPacket(format!(
                    "workCounters.{field} must be zero"
                )));
            }
        }
        let artifact = ArtifactJson::from_serializable(&value).map_err(|error| {
            SearchProjectionError::InvalidPacket(format!(
                "resident graph result canonicalization failed: {error}"
            ))
        })?;
        let semantic_digest = format!("blake3-256:{}", hash_normalized_json(&artifact).value);
        Ok(Self {
            value,
            semantic_digest,
        })
    }

    #[must_use]
    pub fn as_value(&self) -> &Value {
        &self.value
    }

    pub fn into_value(self) -> Value {
        self.value
    }
}

impl SearchProjectionSource for ResidentGraphEvaluationResultV1 {
    fn as_value(&self) -> &Value {
        &self.value
    }

    fn semantic_digest(&self) -> &str {
        &self.semantic_digest
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
        require_string_array(object, "entryNodeIds")?;
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

fn require_bounded_unique_string_array(
    object: &serde_json::Map<String, Value>,
    field: &str,
    max_items: usize,
    max_length: usize,
) -> Result<(), SearchProjectionError> {
    let items = object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| SearchProjectionError::InvalidPacket(format!("{field} must be an array")))?;
    let mut unique = std::collections::HashSet::with_capacity(items.len());
    if items.len() <= max_items
        && items.iter().all(|item| {
            item.as_str().is_some_and(|value| {
                !value.is_empty() && value.len() <= max_length && unique.insert(value)
            })
        })
    {
        return Ok(());
    }
    Err(SearchProjectionError::InvalidPacket(format!(
        "{field} exceeds its bounded unique-string contract"
    )))
}

fn is_language_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn reject_unknown_resident_evaluation_fields(
    object: &serde_json::Map<String, Value>,
) -> Result<(), SearchProjectionError> {
    const FIELDS: &[&str] = &[
        "schemaId",
        "schemaVersion",
        "protocolId",
        "protocolVersion",
        "packetKind",
        "languageId",
        "surface",
        "queryTerms",
        "profile",
        "entryNodeIds",
        "budget",
    ];
    if let Some(field) = object
        .keys()
        .find(|field| !FIELDS.contains(&field.as_str()))
    {
        return Err(SearchProjectionError::InvalidPacket(format!(
            "unknown resident graph evaluation request field: {field}"
        )));
    }
    Ok(())
}

fn validate_resident_evaluation_budget(
    object: &serde_json::Map<String, Value>,
) -> Result<(), SearchProjectionError> {
    let budget = object
        .get("budget")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            SearchProjectionError::InvalidPacket("budget must be an object".to_owned())
        })?;
    const FIELDS: &[&str] = &["maxDepth", "maxNodes", "maxEdges", "maxResults"];
    if budget.len() != FIELDS.len() || budget.keys().any(|field| !FIELDS.contains(&field.as_str()))
    {
        return Err(SearchProjectionError::InvalidPacket(
            "budget must contain only maxDepth, maxNodes, maxEdges, and maxResults".to_owned(),
        ));
    }
    for (field, minimum, maximum) in [
        ("maxDepth", 0, 16),
        ("maxNodes", 1, 256),
        ("maxEdges", 0, 1024),
        ("maxResults", 1, 100),
    ] {
        let value = budget.get(field).and_then(Value::as_u64).ok_or_else(|| {
            SearchProjectionError::InvalidPacket(format!("budget.{field} must be an integer"))
        })?;
        if value < minimum || value > maximum {
            return Err(SearchProjectionError::InvalidPacket(format!(
                "budget.{field} must be between {minimum} and {maximum}"
            )));
        }
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
