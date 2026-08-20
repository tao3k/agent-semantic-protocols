//! Shared agent-facing renderers for typed exact-query projections.

use serde::Serialize;
use serde_json::Value;

/// Renders a callable-skeleton payload with one canonical root selector and
/// packet-local child selector references.
pub fn render_callable_skeleton(payload: &impl Serialize) -> Result<String, String> {
    let payload = serde_json::to_value(payload)
        .map_err(|error| format!("failed to encode callable skeleton payload: {error}"))?;
    let is_semantic_projection = payload.get("schemaId").and_then(Value::as_str)
        == Some("agent.semantic-protocols.semantic-projection")
        && payload.get("schemaVersion").and_then(Value::as_str) == Some("1")
        && payload.get("projectionKind").and_then(Value::as_str) == Some("callable-skeleton")
        && payload.get("payloadSchemaId").and_then(Value::as_str)
            == Some("agent.semantic-protocols.callable-skeleton");
    if !is_semantic_projection {
        return Err("callable skeleton payload has an unsupported schema identity".to_owned());
    }
    let language_id = required_string(&payload, "languageId")?.to_owned();
    let root_selector = required_string(&payload, "rootSelector")?.to_owned();
    let payload = payload
        .get("payload")
        .cloned()
        .ok_or_else(|| "callable skeleton semantic projection is missing payload".to_owned())?;
    let callable = payload
        .get("callable")
        .ok_or_else(|| "callable skeleton payload is missing callable".to_owned())?;
    let display_name = required_string(callable, "displayName")?;
    let nodes = payload
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "callable skeleton payload is missing nodes".to_owned())?;
    let source_bytes = payload
        .get("cost")
        .and_then(|cost| cost.get("sourceBytes"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let compact_display_name = serde_json::to_string(display_name)
        .map_err(|error| format!("failed to encode callable display name: {error}"))?;

    let mut body = String::new();
    body.push_str("R=");
    body.push_str(&root_selector);
    body.push('\n');
    for node in nodes {
        if node.get("nodeId").and_then(Value::as_str) == Some("callable:root") {
            continue;
        }
        let order = node
            .get("order")
            .and_then(Value::as_u64)
            .ok_or_else(|| "callable skeleton node is missing order".to_owned())?;
        let kind = required_string(node, "kind")?;
        let label = required_string(node, "label")?;
        let compact_label = serde_json::to_string(label)
            .map_err(|error| format!("failed to encode callable skeleton label: {error}"))?;
        let selector_ref = match (
            node.get("selector").and_then(Value::as_str),
            node.get("selectorRef").and_then(Value::as_str),
        ) {
            (Some(selector), None) => selector
                .strip_prefix(&root_selector)
                .filter(|suffix| suffix.starts_with('/'))
                .ok_or_else(|| {
                    format!("callable skeleton node selector is outside root selector: {selector}")
                })?,
            (None, Some(selector_ref)) => selector_ref
                .strip_prefix("$root")
                .filter(|suffix| suffix.starts_with('/'))
                .ok_or_else(|| {
                    format!("callable skeleton node selectorRef is invalid: {selector_ref}")
                })?,
            _ => {
                return Err(
                    "queryable callable skeleton node must declare exactly one selector identity"
                        .to_owned(),
                );
            }
        };
        body.push_str(&format!(
            "N{order} kind={} label={} selector=R{}\n",
            compact_atom(kind),
            compact_label,
            selector_ref
        ));
    }
    body.push_str("omit=source,node-authority-copies,expanded-relations\n");

    let mut rendered_bytes = 0usize;
    for _ in 0..16 {
        let omitted_bytes = source_bytes.saturating_sub(rendered_bytes as u64);
        let header = format!(
            "[callable-skeleton] language={} callable={} nodes={} sourceBytes={} renderedBytes={} omittedBytes={}\n",
            compact_atom(&language_id),
            compact_display_name,
            nodes.len(),
            source_bytes,
            rendered_bytes,
            omitted_bytes,
        );
        let next = header.len() + body.len();
        if next == rendered_bytes {
            return Ok(header + &body);
        }
        rendered_bytes = next;
    }
    Err("callable skeleton rendered byte cost did not stabilize".to_owned())
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("callable skeleton payload is missing {field}"))
}

fn compact_atom(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':') {
                character
            } else {
                '_'
            }
        })
        .collect()
}
