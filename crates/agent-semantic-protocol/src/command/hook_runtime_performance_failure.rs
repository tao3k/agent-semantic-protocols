//! Relays typed agent-facing budget failures across the sandbox boundary.

use std::path::Path;

pub(super) async fn relay_post_tool_wall_failure(
    event: &str,
    payload: &serde_json::Value,
    project_root: &Path,
) -> Result<(), String> {
    if event != "post-tool" {
        return Ok(());
    }
    let Some(value) = wall_failure_value(payload)? else {
        return Ok(());
    };
    let Some(_observation) = agent_semantic_client_db::runtime_server_opentelemetry::RuntimePerformanceObservation::from_agent_facing_wall_failure(&value)? else {
        return Ok(());
    };
    crate::command::hook_runtime_memory_inbox::append_runtime_performance_observation(
        project_root,
        value,
    )
    .await?;
    Ok(())
}

fn wall_failure_value(payload: &serde_json::Value) -> Result<Option<serde_json::Value>, String> {
    let candidate = ["tool_response", "toolResponse", "tool_output", "toolOutput"]
        .into_iter()
        .find_map(|field| payload.get(field));
    let Some(candidate) = candidate else {
        return Ok(None);
    };
    if candidate.is_object() {
        return Ok(Some(candidate.clone()));
    }
    let Some(text) = candidate.as_str() else {
        return Ok(None);
    };
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("schemaId").and_then(serde_json::Value::as_str)
            == Some("agent.semantic-protocols.agent-facing-search-wall-failure")
        {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "../../tests/unit/command/hook_runtime_performance_failure.rs"]
mod tests;
