use serde_json::{Value, json};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProviderFactsEnvelope {
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
    pub candidate_annotations: Vec<Value>,
}

pub fn provider_facts_envelope_from_value(value: Value) -> ProviderFactsEnvelope {
    let nodes = value
        .get("nodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let edges = value
        .get("edges")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let candidate_annotations = value
        .get("candidateAnnotations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    ProviderFactsEnvelope {
        nodes,
        edges,
        candidate_annotations,
    }
}

pub fn provider_facts_envelope_from_stdout(stdout: &[u8]) -> Option<ProviderFactsEnvelope> {
    let value = provider_facts_json_from_stdout(stdout)?;
    Some(provider_facts_envelope_from_value(value))
}

fn provider_facts_json_from_stdout(stdout: &[u8]) -> Option<Value> {
    if let Ok(value) = serde_json::from_slice::<Value>(stdout) {
        return Some(value);
    }
    let start = stdout.iter().position(|byte| *byte == b'{')?;
    let end = stdout.iter().rposition(|byte| *byte == b'}')?;
    if end <= start {
        return None;
    }
    serde_json::from_slice::<Value>(&stdout[start..=end]).ok()
}

pub fn compact_provider_fact_nodes(nodes: &[Value]) -> Vec<Value> {
    nodes
        .iter()
        .cloned()
        .map(compact_provider_fact_node)
        .collect()
}

pub fn compact_provider_fact_value(value: &str) -> String {
    let mut first = value.lines().next().unwrap_or(value).trim().to_string();
    if let Some((prefix, _)) = first.split_once(':')
        && !prefix.trim().is_empty()
        && prefix.len() <= 80
    {
        first = prefix.trim().to_string();
    }
    if first.len() > 96 {
        first.truncate(96);
        first.push_str("...");
    }
    first
}

fn compact_provider_fact_node(mut node: Value) -> Value {
    if let Some(value) = node.get("value").and_then(Value::as_str) {
        node["value"] = json!(compact_provider_fact_value(value));
    }
    if let Some(value) = node.get("matchText").and_then(Value::as_str) {
        node["matchText"] = json!(compact_provider_fact_value(value));
    }
    node
}

pub fn provider_candidate_annotation_nodes(annotations: &[Value]) -> Vec<Value> {
    annotations
        .iter()
        .filter_map(provider_candidate_annotation_node)
        .collect()
}

fn provider_candidate_annotation_node(annotation: &Value) -> Option<Value> {
    let path = annotation.get("path").and_then(Value::as_str)?.trim();
    if path.is_empty() {
        return None;
    }
    let attributes = annotation
        .get("attributes")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|attribute| !attribute.is_empty())
        .collect::<Vec<_>>();
    if attributes.is_empty() {
        return None;
    }
    let attribute_value = attributes.join(",");
    let mut node = json!({
        "id": format!("provider-candidate-annotation:{}", stable_annotation_key(path, &attribute_value)),
        "kind": "provider-candidate-annotation",
        "role": "file-attributes",
        "value": attribute_value,
        "action": "evidence",
        "path": path,
        "fields": {
            "path": path,
            "attributes": attributes,
        },
    });
    if let Some(source) = annotation.get("source").and_then(Value::as_str) {
        node["fields"]["source"] = json!(source);
    }
    if let Some(reason) = annotation.get("reason").and_then(Value::as_str) {
        node["fields"]["reason"] = json!(reason);
    }
    Some(node)
}

fn stable_annotation_key(path: &str, attributes: &str) -> String {
    let raw = format!("{path}:{attributes}");
    raw.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
