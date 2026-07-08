//! Compact graph-route receipts for ASP search subagents.

use serde_json::{Value, json};

/// Schema identifier for compact ASP search subagent graph-route receipts.
pub const SEARCH_SUBAGENT_GRAPH_ROUTE_RECEIPT_SCHEMA: &str = "asp.search.subagent.graph-route.v1";

/// Builds a compact graph-route receipt for ASP-managed search subagents.
#[must_use]
pub fn search_subagent_graph_route_receipt(
    intent: impl Into<String>,
    route: impl Into<String>,
    state: impl Into<String>,
    evidence: Vec<Value>,
    next: Value,
) -> Value {
    json!({
        "schema": SEARCH_SUBAGENT_GRAPH_ROUTE_RECEIPT_SCHEMA,
        "intent": intent.into(),
        "route": route.into(),
        "state": state.into(),
        "evidence": evidence,
        "next": next,
    })
}

/// Returns true when a receipt keeps the compact graph-route contract.
#[must_use]
pub fn search_subagent_graph_route_receipt_is_compact(receipt: &Value) -> bool {
    receipt.get("schema").and_then(Value::as_str)
        == Some(SEARCH_SUBAGENT_GRAPH_ROUTE_RECEIPT_SCHEMA)
        && receipt.get("intent").and_then(Value::as_str).is_some()
        && receipt.get("route").and_then(Value::as_str).is_some()
        && receipt.get("state").and_then(Value::as_str).is_some()
        && receipt
            .get("evidence")
            .and_then(Value::as_array)
            .is_some_and(|evidence| !evidence.is_empty())
        && receipt.get("next").and_then(Value::as_object).is_some()
        && !contains_forbidden_receipt_shape(receipt)
}

fn contains_forbidden_receipt_shape(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            forbidden_receipt_key(key) || contains_forbidden_receipt_shape(value)
        }),
        Value::Array(values) => values.iter().any(contains_forbidden_receipt_shape),
        Value::String(text) => text.contains('\n') || text.len() > 512,
        _ => false,
    }
}

fn forbidden_receipt_key(key: &str) -> bool {
    matches!(
        key,
        "body"
            | "code"
            | "commandLog"
            | "confidence"
            | "displayLineRange"
            | "endLine"
            | "explanation"
            | "lineRange"
            | "snippet"
            | "sourceBody"
            | "sourceLocatorHint"
            | "startLine"
    )
}
