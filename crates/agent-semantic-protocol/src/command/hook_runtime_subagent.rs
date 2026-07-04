use std::fs;
use std::path::{Path, PathBuf};

const CODEX_DEFAULT_RESIDENT_AGENT_MODEL: &str = "gpt-5.4-mini";
const CLAUDE_DEFAULT_RESIDENT_AGENT_MODEL: &str = "haiku";

pub(super) fn subagent_model_arg(client: &str, model: Option<&str>) -> Result<String, String> {
    let model = match model {
        Some(value) => value.trim(),
        None => default_subagent_model(client),
    };
    validate_subagent_model(model)?;
    Ok(model.to_string())
}

pub(super) fn install_claude_resident_agents(
    project_root: &Path,
    subagent_model: &str,
) -> Result<PathBuf, String> {
    let contents = claude_resident_search_agent(subagent_model)?;
    let canonical_path = asp_agent_config_path("asp-explorer", "claude", "md")?;
    write_agent_config(&canonical_path, contents.as_bytes())?;
    let path = project_root
        .join(".claude")
        .join("agents")
        .join("asp-explorer.md");
    project_agent_config(&canonical_path, &path)?;
    let testing_contents = claude_resident_testing_agent(subagent_model)?;
    let testing_canonical_path = asp_agent_config_path("asp-testing", "claude", "md")?;
    write_agent_config(&testing_canonical_path, testing_contents.as_bytes())?;
    let testing_path = project_root
        .join(".claude")
        .join("agents")
        .join("asp-testing.md");
    project_agent_config(&testing_canonical_path, &testing_path)?;
    Ok(path)
}

pub(super) fn install_codex_resident_agents(
    codex_home: &Path,
    subagent_model: &str,
) -> Result<PathBuf, String> {
    let contents = codex_resident_search_agent(subagent_model)?;
    let canonical_path = asp_agent_config_path("asp-explorer", "codex", "toml")?;
    write_agent_config(&canonical_path, contents.as_bytes())?;
    let path = codex_home.join("agents").join("asp-explorer.toml");
    project_agent_config(&canonical_path, &path)?;
    let testing_contents = codex_resident_testing_agent(subagent_model)?;
    let testing_canonical_path = asp_agent_config_path("asp-testing", "codex", "toml")?;
    write_agent_config(&testing_canonical_path, testing_contents.as_bytes())?;
    let testing_path = codex_home.join("agents").join("asp-testing.toml");
    project_agent_config(&testing_canonical_path, &testing_path)?;
    Ok(path)
}

fn asp_agent_config_path(name: &str, client: &str, extension: &str) -> Result<PathBuf, String> {
    Ok(agent_semantic_runtime::state_core::resolve_state_home()?
        .join("agents")
        .join(format!("{name}_{client}.{extension}")))
}

fn write_agent_config(path: &Path, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(())
}

fn project_agent_config(source: &Path, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    match fs::symlink_metadata(target) {
        Ok(metadata) => {
            if metadata.is_dir() {
                return Err(format!("cannot replace directory {}", target.display()));
            }
            fs::remove_file(target)
                .map_err(|error| format!("failed to replace {}: {error}", target.display()))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!("failed to inspect {}: {error}", target.display()));
        }
    }
    link_or_copy_agent_config(source, target)
}

#[cfg(unix)]
fn link_or_copy_agent_config(source: &Path, target: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(source, target).map_err(|error| {
        format!(
            "failed to symlink {} -> {}: {error}",
            target.display(),
            source.display()
        )
    })
}

#[cfg(not(unix))]
fn link_or_copy_agent_config(source: &Path, target: &Path) -> Result<(), String> {
    fs::copy(source, target).map(|_| ()).map_err(|error| {
        format!(
            "failed to copy {} -> {}: {error}",
            source.display(),
            target.display()
        )
    })
}

fn default_subagent_model(client: &str) -> &'static str {
    match client {
        "codex" => CODEX_DEFAULT_RESIDENT_AGENT_MODEL,
        "claude" => CLAUDE_DEFAULT_RESIDENT_AGENT_MODEL,
        _ => unreachable!("client support checked before model default"),
    }
}

fn codex_resident_search_agent(subagent_model: &str) -> Result<String, String> {
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
        resident_search_instructions()
    ))
}

fn codex_resident_testing_agent(subagent_model: &str) -> Result<String, String> {
    Ok(format!(
        r#"name = "asp_testing"
description = "ASP test/build execution lane."
nickname_candidates = ["ASP test", "ASP check", "ASP build"]
model = {}
model_reasoning_effort = "medium"
sandbox_mode = "workspace-write"
developer_instructions = """
Run only ASP-routed test, check, build, and compile commands for the current project.
Do not edit files. Return compact evidence: command, exit status, failing target,
first actionable error, and next command when useful.
"""
"#,
        toml_basic_string(subagent_model)?
    ))
}

fn claude_resident_search_agent(subagent_model: &str) -> Result<String, String> {
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
        resident_search_instructions()
    ))
}

fn claude_resident_testing_agent(subagent_model: &str) -> Result<String, String> {
    Ok(format!(
        r#"---
name: asp-testing
description: ASP test/build execution lane.
tools: Bash, Read, Glob, Grep
model: {}
permissionMode: acceptEdits
maxTurns: 8
---

Run only ASP-routed test, check, build, and compile commands for the current project.
Do not edit files. Return compact evidence: command, exit status, failing target,
first actionable error, and next command when useful.
"#,
        yaml_single_quoted(subagent_model)?
    ))
}

fn resident_search_instructions() -> &'static str {
    r#"You are the ASP explorer.

Do not edit files.
Use ASP provider commands before source reads, and prefer parser-owned owner, selector, and query-language routes.
Follow ASP recommendedNext or nextCommand when present; stop retrying a command after a hook denial.
Return compact evidence for the assigned branch; do not spawn subagents.

Prefer:
- asp <language> search lexical '<term-or-error>' owner tests --workspace . --view seeds
- asp fd -query '<owner-or-path terms>' .
- asp rg -query '<content-or-error terms>' .
- asp <language> search owner <owner-path> items --query '<symbol-or-a|b|c>' --workspace . --view seeds
- asp <language> search pipe '<refinement-query>' --workspace . --view seeds only after lexical/dependency evidence is ambiguous
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
