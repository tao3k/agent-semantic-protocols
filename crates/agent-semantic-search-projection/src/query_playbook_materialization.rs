//! Replay-safe handoff from one admitted Search settlement to Query Playbook.

use std::fmt;

use serde_json::Map;
use serde_json::Value;

pub const QUERY_PLAYBOOK_MATERIALIZATION_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.query-playbook-materialization-request";
pub const QUERY_PLAYBOOK_MATERIALIZATION_REQUEST_SCHEMA_VERSION: &str = "1";

/// One immutable Query Playbook request derived from Search MaterializationSet.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryPlaybookMaterializationRequest {
    packet: Value,
}

impl QueryPlaybookMaterializationRequest {
    /// Admit only an exact projection of the originating Search settlement.
    pub fn admit_for_settlement(
        packet: Value,
        settlement: &crate::SearchTopologySettlement,
    ) -> Result<Self, QueryPlaybookMaterializationError> {
        let packet_object = object(&packet, "packet")?;
        text_eq(
            packet_object,
            "schemaId",
            QUERY_PLAYBOOK_MATERIALIZATION_REQUEST_SCHEMA_ID,
        )?;
        text_eq(
            packet_object,
            "schemaVersion",
            QUERY_PLAYBOOK_MATERIALIZATION_REQUEST_SCHEMA_VERSION,
        )?;
        text_eq(
            packet_object,
            "protocolId",
            "agent.semantic-protocols.query-playbook",
        )?;
        text_eq(packet_object, "protocolVersion", "1")?;
        match text(packet_object, "projection")? {
            "matches" | "source" | "callable-skeleton" => {}
            _ => return invalid("schema-invalid", "unsupported Query projection"),
        }

        let settlement_packet = object(settlement.as_json(), "settlement")?;
        let expected_binding = object_field(settlement_packet, "binding")?;
        let actual_binding = object_field(packet_object, "settlementBinding")?;
        for field in [
            "workspaceIdentity",
            "sourceGenerationDigest",
            "providerCatalogDigest",
            "topologyLibraryDigest",
            "topologyGenerationDigest",
            "structuralTopologyDigest",
            "semanticTopologyDigest",
            "inferenceProgramDigest",
        ] {
            if text(actual_binding, field)? != text(expected_binding, field)? {
                return invalid(
                    "query-playbook-binding-mismatch",
                    format!("Query handoff differs at {field}"),
                );
            }
        }

        let expected = object_field(settlement_packet, "materializationSet")?;
        if text(packet_object, "searchRequestId")? != text(expected, "requestId")? {
            return invalid(
                "query-playbook-request-mismatch",
                "Query handoff names another Search request",
            );
        }
        if text(packet_object, "materializationSetDigest")? != text(expected, "digest")?
            || packet_object.get("selectors") != expected.get("selectors")
            || packet_object.get("proofDependencies") != expected.get("proofDependencies")
        {
            return invalid(
                "query-playbook-materialization-mismatch",
                "Query handoff is not the exact Search MaterializationSet",
            );
        }

        let selectors = array(packet_object, "selectors")?;
        if selectors.is_empty()
            || selectors.iter().any(|selector| {
                selector
                    .as_str()
                    .is_none_or(|value| !value.contains("://") || !value.contains("#item/"))
            })
            || selectors.windows(2).any(|pair| {
                pair[0].as_str().expect("validated selector text")
                    >= pair[1].as_str().expect("validated selector text")
            })
        {
            return invalid(
                "query-playbook-materialization-mismatch",
                "Query selectors must be unique canonical selectors in stable order",
            );
        }

        Ok(Self { packet })
    }

    pub fn as_json(&self) -> &Value {
        &self.packet
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryPlaybookMaterializationError {
    reason_kind: &'static str,
    message: String,
}

impl QueryPlaybookMaterializationError {
    pub fn reason_kind(&self) -> &'static str {
        self.reason_kind
    }
}

impl fmt::Display for QueryPlaybookMaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind, self.message)
    }
}

impl std::error::Error for QueryPlaybookMaterializationError {}

fn error(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> QueryPlaybookMaterializationError {
    QueryPlaybookMaterializationError {
        reason_kind,
        message: message.into(),
    }
}

fn invalid<T>(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> Result<T, QueryPlaybookMaterializationError> {
    Err(error(reason_kind, message))
}

fn object<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a Map<String, Value>, QueryPlaybookMaterializationError> {
    value
        .as_object()
        .ok_or_else(|| error("schema-invalid", format!("{field} must be an object")))
}

fn object_field<'a>(
    map: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Map<String, Value>, QueryPlaybookMaterializationError> {
    object(map.get(field).unwrap_or(&Value::Null), field)
}

fn text<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, QueryPlaybookMaterializationError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| error("schema-invalid", format!("{field} must be non-empty text")))
}

fn text_eq(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), QueryPlaybookMaterializationError> {
    if text(object, field)? == expected {
        Ok(())
    } else {
        invalid(
            "schema-invalid",
            format!("{field} has an unsupported value"),
        )
    }
}

fn array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Vec<Value>, QueryPlaybookMaterializationError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| error("schema-invalid", format!("{field} must be an array")))
}
