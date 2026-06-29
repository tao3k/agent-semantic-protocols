use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const CODEX_ASP_EXPLORER_MODEL: &str = "gpt-5.3-codex-spark";
const CLAUDE_ASP_EXPLORER_MODEL: &str = "haiku";

pub(super) fn subagent_model_arg(client: &str, model: Option<&str>) -> Result<String, String> {
    let model = match model {
        Some(value) => value.trim(),
        None => default_subagent_model(client),
    };
    validate_subagent_model(model)?;
    Ok(model.to_string())
}

pub(super) fn install_claude_asp_explorer_agent(
    project_root: &Path,
    subagent_model: &str,
) -> Result<PathBuf, String> {
    let path = project_root
        .join(".claude")
        .join("agents")
        .join("asp-explorer.md");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        remove_stale_agent_files(
            parent,
            &[
                "asp-explorer-owner.md",
                "asp-explorer-rg.md",
                "asp-explorer-selector.md",
            ],
        )?;
    }
    let contents = claude_asp_explorer_agent(subagent_model)?;
    fs::write(&path, contents.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(path)
}

pub(super) fn install_codex_asp_explorer_agent(
    project_root: &Path,
    subagent_model: &str,
) -> Result<PathBuf, String> {
    let path = project_root
        .join(".codex")
        .join("agents")
        .join("asp-explorer.toml");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        remove_stale_agent_files(
            parent,
            &[
                "asp-explorer-owner.toml",
                "asp-explorer-rg.toml",
                "asp-explorer-selector.toml",
            ],
        )?;
    }
    let contents = codex_asp_explorer_agent(subagent_model)?;
    fs::write(&path, contents.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(path)
}

fn default_subagent_model(client: &str) -> &'static str {
    match client {
        "codex" => CODEX_ASP_EXPLORER_MODEL,
        "claude" => CLAUDE_ASP_EXPLORER_MODEL,
        _ => unreachable!("client support checked before model default"),
    }
}

fn remove_stale_agent_files(parent: &Path, file_names: &[&str]) -> Result<(), String> {
    for file_name in file_names {
        let path = parent.join(file_name);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to remove stale generated subagent {}: {error}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn codex_asp_explorer_agent(subagent_model: &str) -> Result<String, String> {
    Ok(format!(
        r#"name = "asp_explorer"
description = "ASP search/query evidence explorer."
nickname_candidates = ["ASP owner", "ASP rg", "ASP selector", "ASP search"]
model = {}
model_reasoning_effort = "medium"
sandbox_mode = "read-only"
developer_instructions = """
{}
"""
"#,
        toml_basic_string(subagent_model)?,
        asp_explorer_instructions()
    ))
}

fn claude_asp_explorer_agent(subagent_model: &str) -> Result<String, String> {
    Ok(format!(
        r#"---
name: asp-explorer
description: ASP search/query evidence explorer.
tools: Bash, Read, Glob, Grep
model: {}
permissionMode: plan
maxTurns: 8
---

{}
"#,
        yaml_single_quoted(subagent_model)?,
        asp_explorer_instructions()
    ))
}

fn asp_explorer_instructions() -> &'static str {
    r#"You are the ASP explorer.

Do not edit files.
Use ASP provider commands before source reads, and prefer parser-owned owner, selector, and query-language routes.
Follow ASP recommendedNext or nextCommand when present; stop retrying a command after a hook denial.
Return compact evidence for the assigned branch; do not spawn subagents.

Prefer:
- asp <language> search pipe '<question-or-feature-term>' --workspace . --view seeds
- asp fd -query '<owner-or-path terms>' .
- asp rg -query '<content-or-error terms>' .
- asp <language> search owner <owner-path> items --query '<symbol-or-a|b|c>' --workspace . --view seeds
- asp <language> query --selector <exact-selector> --workspace . --code

Return one compact receipt line:
[asp-search-subagent] role=<assigned branch role> action=<action id or -> evidence=<compact facts> missing=<missing facts or -> next=<recommended next action> risk=<risk or ->.

Return actionFrontier, recommendedNext, risk, missing facts, and exact selectors only when confidence is high."#
}

fn validate_subagent_model(model: &str) -> Result<(), String> {
    if model.trim().is_empty() {
        return Err("--subagent-model must not be empty".to_string());
    }
    if model.chars().any(char::is_control) {
        return Err("--subagent-model must not contain control characters".to_string());
    }
    Ok(())
}

fn yaml_single_quoted(value: &str) -> Result<String, String> {
    validate_subagent_model(value)?;
    Ok(format!("'{}'", value.replace('\'', "''")))
}

fn toml_basic_string(value: &str) -> Result<String, String> {
    validate_subagent_model(value)?;
    Ok(format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}
