use std::fs;
use std::path::{Path, PathBuf};

const CODEX_FALLBACK_RESIDENT_AGENT_MODEL: &str = "gpt-5.4-mini";
const CLAUDE_DEFAULT_RESIDENT_AGENT_MODEL: &str = "haiku";

pub(super) fn subagent_model_arg(client: &str, model: Option<&str>) -> Result<String, String> {
    let model = match model {
        Some(value) => value.trim().to_string(),
        None => default_subagent_model(client)?,
    };
    validate_subagent_model(&model)?;
    Ok(model)
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

pub(crate) fn install_codex_resident_agents(
    codex_home: &Path,
    subagent_model: &str,
) -> Result<PathBuf, String> {
    let path = refresh_codex_resident_search_agent(codex_home, subagent_model)?;
    let testing_contents = codex_resident_testing_agent(subagent_model)?;
    let testing_canonical_path = asp_agent_config_path("asp-testing", "codex", "toml")?;
    write_agent_config(&testing_canonical_path, testing_contents.as_bytes())?;
    let testing_path = codex_home.join("agents").join("asp-testing.toml");
    project_agent_config(&testing_canonical_path, &testing_path)?;
    Ok(path)
}

pub(crate) fn refresh_codex_resident_search_agent(
    codex_home: &Path,
    subagent_model: &str,
) -> Result<PathBuf, String> {
    let contents = codex_resident_search_agent(subagent_model)?;
    let canonical_path = asp_agent_config_path("asp-explorer", "codex", "toml")?;
    write_agent_config(&canonical_path, contents.as_bytes())?;
    let path = codex_home.join("agents").join("asp-explorer.toml");
    project_agent_config(&canonical_path, &path)?;
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

fn default_subagent_model(client: &str) -> Result<String, String> {
    match client {
        "codex" => codex_default_subagent_model(),
        "claude" => Ok(CLAUDE_DEFAULT_RESIDENT_AGENT_MODEL.to_string()),
        _ => unreachable!("client support checked before model default"),
    }
}

fn codex_default_subagent_model() -> Result<String, String> {
    let config_path = agent_semantic_runtime::state_core::resolve_state_home()?
        .join("agents")
        .join("config.toml");
    Ok(read_codex_primary_model(&config_path)?
        .unwrap_or_else(|| CODEX_FALLBACK_RESIDENT_AGENT_MODEL.to_string()))
}

fn read_codex_primary_model(config_path: &Path) -> Result<Option<String>, String> {
    if !config_path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(config_path)
        .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
    let value = toml::from_str::<toml::Value>(&text)
        .map_err(|error| format!("failed to parse {}: {error}", config_path.display()))?;
    let model = value
        .get("platform")
        .and_then(toml::Value::as_table)
        .and_then(|platform| platform.get("codex"))
        .and_then(toml::Value::as_table)
        .and_then(|codex| codex.get("models"))
        .and_then(toml::Value::as_table)
        .and_then(|models| models.get("primary"))
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(ToString::to_string);
    Ok(model)
}

fn codex_resident_search_agent(subagent_model: &str) -> Result<String, String> {
    render_codex_agent(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../agents/asp-explorer_codex.toml"
        )),
        subagent_model,
    )
}

fn codex_resident_testing_agent(subagent_model: &str) -> Result<String, String> {
    render_codex_agent(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../agents/asp-testing_codex.toml"
        )),
        subagent_model,
    )
}

fn claude_resident_search_agent(subagent_model: &str) -> Result<String, String> {
    render_claude_agent(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../agents/asp-explorer_claude.md"
        )),
        subagent_model,
    )
}

fn claude_resident_testing_agent(subagent_model: &str) -> Result<String, String> {
    render_claude_agent(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../agents/asp-testing_claude.md"
        )),
        subagent_model,
    )
}

fn render_codex_agent(template: &str, model: &str) -> Result<String, String> {
    Ok(template.replace("{{MODEL_TOML}}", &toml_basic_string(model)?))
}

fn render_claude_agent(template: &str, model: &str) -> Result<String, String> {
    Ok(template.replace("{{MODEL_YAML}}", &yaml_single_quoted(model)?))
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
