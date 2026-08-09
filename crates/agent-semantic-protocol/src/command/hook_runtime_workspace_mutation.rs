use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
struct HookWorkspaceMutation {
    mutation_id: String,
    changed_paths: Vec<String>,
}

fn post_tool_workspace_mutation(
    event: &str,
    payload: &serde_json::Value,
) -> Result<Option<HookWorkspaceMutation>, String> {
    if event != "post-tool" {
        return Ok(None);
    }
    let tool_name = payload
        .get("tool_name")
        .or_else(|| payload.get("toolName"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "post-tool mutation payload omitted typed tool name".to_owned())?;
    let tool_input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))
        .unwrap_or(&serde_json::Value::Null);
    let changed_paths = agent_semantic_hook::workspace_mutation_paths(tool_name, tool_input);
    if changed_paths.is_empty() {
        return Ok(None);
    }
    let mutation_id = ["tool_use_id", "toolUseId", "hook_run_id", "hookRunId"]
        .into_iter()
        .find_map(|key| payload.get(key).and_then(serde_json::Value::as_str))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "post-tool workspace mutation omitted typed mutation identity".to_owned())?;
    Ok(Some(HookWorkspaceMutation {
        mutation_id: mutation_id.to_owned(),
        changed_paths,
    }))
}

pub(super) async fn relay_post_tool_workspace_mutation(
    event: &str,
    payload: &serde_json::Value,
    project_root: &Path,
) -> Result<(), String> {
    let Some(mutation) = post_tool_workspace_mutation(event, payload)? else {
        return Ok(());
    };
    crate::command::hook_runtime_memory_inbox::append_workspace_mutation(
        project_root,
        &mutation.mutation_id,
        mutation.changed_paths,
    )
    .await?;
    if std::env::var_os("ASP_HOOK_BOOTSTRAP_TRACE").is_some() {
        eprintln!("[asp-hook] route=local-mmap-inbox entryKind=workspace-mutation state=recorded");
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/command/hook_runtime_workspace_mutation.rs"]
mod tests;
