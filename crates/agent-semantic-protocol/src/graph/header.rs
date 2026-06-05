use serde_json::Value;

use super::packet::{header_field_scalar, packet_query};

pub(crate) fn graph_header(
    packet: &Value,
    mode: &str,
    root: &str,
    algorithm: &str,
    owner_item_query_packet: bool,
    term_count: Option<usize>,
) -> String {
    let mut fields = Vec::new();
    fields.push(format!("[search-{}]", header_mode(mode)));
    if owner_item_query_packet {
        fields.push(format!("owner={}", packet_query(packet).unwrap_or(root)));
    } else if let Some(query) = packet_query(packet) {
        fields.push(format!("q={query}"));
    } else {
        fields.push(format!("root={root}"));
    }
    if owner_item_query_packet {
        fields.push("selector=items".to_string());
        if let Some(terms) = term_count {
            fields.push(format!("terms={terms}"));
        }
        fields.push("view=seeds".to_string());
    }
    for key in [
        "querySet",
        "selector",
        "scope",
        "view",
        "analysis",
        "nativeSyntaxFacts",
        "policyFindings",
    ] {
        if owner_item_query_packet && matches!(key, "selector" | "view") {
            continue;
        }
        if key == "querySet" {
            if let Some(count) = packet
                .get("querySet")
                .and_then(Value::as_array)
                .map(Vec::len)
            {
                fields.push(format!("{key}={count}"));
            }
            continue;
        }
        if let Some(value) = header_field_string(packet, key) {
            fields.push(format!("{key}={value}"));
        }
    }
    fields.push(format!("alg={algorithm}"));
    fields.join(" ")
}

fn header_mode(mode: &str) -> &str {
    match mode {
        "deps" => "dependency",
        other => other,
    }
}

fn header_field_string(packet: &Value, key: &str) -> Option<String> {
    if key == "querySet"
        && let Some(count) = packet
            .get("querySet")
            .and_then(Value::as_array)
            .map(Vec::len)
            .filter(|count| *count > 0)
    {
        return Some(count.to_string());
    }
    packet
        .get("header")
        .and_then(|header| header.get("fields"))
        .and_then(|fields| fields.get(key))
        .and_then(header_field_scalar)
}
