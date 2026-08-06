//! Relays typed agent-facing budget failures across the sandbox boundary.

use std::path::Path;

pub(super) fn relay_post_tool_wall_failure(
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
    let Some(mut observation) = agent_semantic_client_db::runtime_server_opentelemetry::RuntimePerformanceObservation::from_agent_facing_wall_failure(&value)? else {
        return Ok(());
    };
    let state_home = crate::server::runtime_server::state_home()?;
    let catalog_path = state_home
        .join("runtime")
        .join("server")
        .join("workspace-admissions.v1.json");
    let admission = agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog::resolve_mapped(
        &catalog_path,
        project_root,
    )
    .map_err(|error| format!("performance failure workspace admission is unavailable: {error}"))?;
    observation.workspace_identity = Some(admission.workspace_identity);
    observation.seal_budget_failure_identity();
    let socket_path =
        crate::server::runtime_server::runtime_server_telemetry_socket_path(&state_home);
    let runtime =
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerClientExecutor::get()?;
    let receipt = runtime.block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_millis(25),
            agent_semantic_client_db::runtime_server_opentelemetry::admit_to_runtime(
                &socket_path,
                &observation,
            ),
        )
        .await
        .map_err(|_| "Runtime Server performance ingress exceeded 25ms".to_owned())?
    });
    let receipt = receipt?;
    if std::env::var_os("ASP_HOOK_BOOTSTRAP_TRACE").is_some() {
        eprintln!(
            "[asp-hook] route=performance-failure-relay state={} workspaceIdentity={} surface={} stage={}",
            receipt.state, receipt.workspace_identity, receipt.surface, receipt.stage
        );
    }
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
