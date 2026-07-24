use serde_json::Value;

/// Resolve the topology root from typed packet facts.
pub(super) fn graph_root(packet: &Value, mode: &str) -> String {
    if matches!(mode, "prime" | "package") {
        return packet_string(packet, &["header", "fields", "package"])
            .or_else(|| packet_string(packet, &["header", "fields", "pkg"]))
            .or_else(|| packet_string(packet, &["projectRoot"]))
            .unwrap_or_else(|| ".".to_string());
    }
    packet_query(packet)
        .filter(|query| !query.contains(char::is_whitespace))
        .unwrap_or(".")
        .to_string()
}

pub(super) fn packet_view(packet: &Value) -> &str {
    packet
        .get("view")
        .and_then(Value::as_str)
        .or_else(|| {
            packet
                .get("header")
                .and_then(|header| header.get("kind"))
                .and_then(Value::as_str)
                .and_then(|kind| kind.strip_prefix("search-"))
        })
        .unwrap_or("search")
}

pub(super) fn packet_query(packet: &Value) -> Option<&str> {
    packet
        .get("query")
        .and_then(Value::as_str)
        .or_else(|| {
            packet
                .get("header")
                .and_then(|header| header.get("fields"))
                .and_then(|fields| fields.get("q"))
                .and_then(Value::as_str)
        })
        .filter(|query| !query.trim().is_empty())
}

pub(super) fn fallback_algorithm(mode: &str) -> String {
    match mode {
        "prime" | "package" => "budgeted-prime-frontier-v1",
        "query" | "lexical" => "native-syntax-query",
        _ => "seed-frontier",
    }
    .to_string()
}

pub(super) fn packet_string(packet: &Value, path: &[&str]) -> Option<String> {
    let mut current = packet;
    for segment in path {
        current = current.get(*segment)?;
    }
    header_field_scalar(current)
}

pub(super) fn is_owner_item_query_packet(packet: &Value, mode: &str) -> bool {
    mode == "owner"
        && packet
            .get("items")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
}

pub(super) fn header_field_scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(value) if !value.trim().is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}
