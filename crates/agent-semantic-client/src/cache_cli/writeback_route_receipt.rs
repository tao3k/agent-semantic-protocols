//! Search route receipt writeback bridge for DB Engine backends.

use std::path::Path;

use sha2::{Digest, Sha256};

/// Persist a bounded route receipt for a validated search packet when Turso is active.
pub(super) fn maybe_write_turso_route_receipt_for_search_packet(
    project_root: &Path,
    packet_bytes: &[u8],
    rendered_stdout: &[u8],
) -> Option<()> {
    use std::time::{SystemTime, UNIX_EPOCH};

    use agent_semantic_client_core::state_core::ResolvedState;
    use agent_semantic_client_db::{ClientDbEngine, TursoClientDbRouteReceipt};
    use agent_semantic_runtime::runtime_block_on_current_thread;
    use serde_json::Value;
    let packet: Value = serde_json::from_slice(packet_bytes).ok()?;
    let query = packet.get("query")?.as_str()?.trim();
    if query.is_empty() {
        return None;
    }
    let state = ResolvedState::resolve(project_root).ok()?;
    state.ensure_minimal_layout().ok()?;
    let engine = ClientDbEngine::from_resolved_state(&state);
    let route_source = route_receipt_source(&packet);
    let selected_selector = selected_selector(&packet);
    let next_command = packet
        .get("nextCommand")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| rendered_next_command(rendered_stdout));
    let evidence_ids = route_evidence_ids(&packet);
    let hit_count = hit_count(&packet, evidence_ids.len());
    let created_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis()
        .min(i64::MAX as u128) as i64;
    let receipt_id = route_receipt_id(
        state.repo.repo_id.as_str(),
        state.workspace.workspace_id.as_str(),
        query,
        &route_source,
        selected_selector.as_deref(),
        next_command.as_deref(),
        &evidence_ids,
    );
    let receipt = TursoClientDbRouteReceipt {
        receipt_id,
        repo_id: state.repo.repo_id.to_string(),
        workspace_id: state.workspace.workspace_id.to_string(),
        scope_id: state.scope_id.to_string(),
        session_id: None,
        query: query.to_string(),
        route_source,
        selected_selector,
        next_command,
        hit_count,
        evidence_ids,
        created_at_ms,
    };
    runtime_block_on_current_thread(async move { engine.upsert_route_receipt(&receipt).await })
        .ok()?
        .ok()
}

fn route_receipt_source(packet: &serde_json::Value) -> String {
    if let Some(source) = packet
        .get("routeSource")
        .and_then(serde_json::Value::as_str)
        && matches!(
            source,
            "context-anchor"
                | "overlay-fts"
                | "stable-fts"
                | "lexical-control"
                | "source-index"
                | "graph-route"
                | "semantic-vector"
                | "manual"
        )
    {
        return source.to_string();
    }
    if packet
        .get("searchSynthesis")
        .and_then(serde_json::Value::as_object)
        .is_some()
        || packet
            .get("nodes")
            .and_then(serde_json::Value::as_array)
            .is_some()
    {
        "graph-route".to_string()
    } else if packet
        .get("hits")
        .and_then(serde_json::Value::as_array)
        .is_some()
    {
        "source-index".to_string()
    } else {
        "manual".to_string()
    }
}

fn selected_selector(packet: &serde_json::Value) -> Option<String> {
    packet
        .get("selectedSelector")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| first_array_string(packet, "hits", "selector"))
        .or_else(|| first_array_string(packet, "nodes", "selector"))
        .or_else(|| first_array_string(packet, "owners", "selector"))
}

fn first_array_string(packet: &serde_json::Value, array_key: &str, field: &str) -> Option<String> {
    packet
        .get(array_key)
        .and_then(serde_json::Value::as_array)?
        .iter()
        .find_map(|value| value.get(field)?.as_str().map(ToOwned::to_owned))
}

fn rendered_next_command(rendered_stdout: &[u8]) -> Option<String> {
    let stdout = std::str::from_utf8(rendered_stdout).ok()?;
    let marker = "next=\"";
    let start = stdout.find(marker)? + marker.len();
    let rest = &stdout[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn route_evidence_ids(packet: &serde_json::Value) -> Vec<String> {
    let mut ids = Vec::new();
    for (array_key, fields) in [
        ("hits", ["id", "selector", "path"]),
        ("nodes", ["id", "selector", "path"]),
        ("owners", ["id", "selector", "path"]),
    ] {
        if let Some(values) = packet.get(array_key).and_then(serde_json::Value::as_array) {
            for value in values {
                for field in fields {
                    if let Some(text) = value.get(field).and_then(serde_json::Value::as_str)
                        && !text.is_empty()
                    {
                        ids.push(format!("{array_key}:{field}:{text}"));
                        break;
                    }
                }
            }
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

fn hit_count(packet: &serde_json::Value, evidence_count: usize) -> u32 {
    for key in ["hits", "owners", "nodes"] {
        if let Some(values) = packet.get(key).and_then(serde_json::Value::as_array) {
            return values.len().min(u32::MAX as usize) as u32;
        }
    }
    evidence_count.min(u32::MAX as usize) as u32
}

fn route_receipt_id(
    repo_id: &str,
    workspace_id: &str,
    query: &str,
    route_source: &str,
    selected_selector: Option<&str>,
    next_command: Option<&str>,
    evidence_ids: &[String],
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        repo_id,
        workspace_id,
        query,
        route_source,
        selected_selector.unwrap_or(""),
        next_command.unwrap_or(""),
    ] {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    for evidence_id in evidence_ids {
        hasher.update(evidence_id.as_bytes());
        hasher.update([0]);
    }
    format!("route-receipt:{:x}", hasher.finalize())
}
