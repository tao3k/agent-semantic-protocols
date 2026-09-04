//! Validation of Host-specific agent projection documents.

use std::path::Path;

pub(super) fn projection_string(
    value: &toml::Value,
    field: &str,
    path: &Path,
) -> Result<String, String> {
    value
        .get(field)
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("projection {} requires `{field}`", path.display()))
}

pub(super) fn validate_codex_agent_projection(
    value: &toml::Value,
    path: &Path,
) -> Result<(), String> {
    const ALLOWED: &[&str] = &[
        "name",
        "description",
        "nickname_candidates",
        "model",
        "model_reasoning_effort",
        "sandbox_mode",
        "developer_instructions",
    ];
    let table = value
        .as_table()
        .ok_or_else(|| format!("Codex projection {} must be a TOML table", path.display()))?;
    if let Some(unknown) = table.keys().find(|key| !ALLOWED.contains(&key.as_str())) {
        return Err(format!(
            "Codex projection {} declares unknown `{unknown}` field",
            path.display()
        ));
    }
    let name = projection_string(value, "name", path)?;
    let mut chars = name.chars();
    let valid_name = chars.next().is_some_and(|ch| ch.is_ascii_lowercase())
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_');
    if !valid_name {
        return Err(format!(
            "Codex projection {} has invalid agent name `{name}`",
            path.display()
        ));
    }
    projection_string(value, "description", path)?;
    projection_string(value, "model", path)?;
    projection_string(value, "developer_instructions", path)?;
    let reasoning = projection_string(value, "model_reasoning_effort", path)?;
    if !matches!(
        reasoning.as_str(),
        "low" | "medium" | "high" | "xhigh" | "max"
    ) {
        return Err(format!(
            "Codex projection {} has invalid model_reasoning_effort `{reasoning}`",
            path.display()
        ));
    }
    let sandbox = projection_string(value, "sandbox_mode", path)?;
    if !matches!(
        sandbox.as_str(),
        "read-only" | "workspace-write" | "danger-full-access"
    ) {
        return Err(format!(
            "Codex projection {} has invalid sandbox_mode `{sandbox}`",
            path.display()
        ));
    }
    Ok(())
}

pub(super) fn markdown_frontmatter_value(source: &str, key: &str) -> Option<String> {
    let mut lines = source.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines {
        let line = line.trim();
        if line == "---" {
            break;
        }
        let Some((candidate, value)) = line.split_once(':') else {
            continue;
        };
        if candidate.trim() == key {
            return Some(value.trim().to_owned());
        }
    }
    None
}

fn markdown_frontmatter_keys(source: &str) -> Option<Vec<String>> {
    let mut lines = source.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut keys = Vec::new();
    for line in lines {
        let line = line.trim();
        if line == "---" {
            return Some(keys);
        }
        if let Some((key, _)) = line.split_once(':') {
            keys.push(key.trim().to_owned());
        }
    }
    None
}

pub(super) fn validate_claude_plugin_projection(source: &str, path: &Path) -> Result<(), String> {
    const ALLOWED: &[&str] = &[
        "name",
        "description",
        "tools",
        "disallowedTools",
        "model",
        "maxTurns",
        "skills",
        "memory",
        "background",
        "effort",
        "isolation",
        "color",
        "initialPrompt",
    ];
    let keys = markdown_frontmatter_keys(source).ok_or_else(|| {
        format!(
            "Claude plugin projection {} requires closed YAML frontmatter",
            path.display()
        )
    })?;
    if let Some(unknown) = keys.iter().find(|key| !ALLOWED.contains(&key.as_str())) {
        return Err(format!(
            "Claude plugin projection {} declares unknown or ignored `{unknown}` frontmatter",
            path.display()
        ));
    }
    let name = markdown_frontmatter_value(source, "name")
        .ok_or_else(|| format!("Claude projection {} requires `name`", path.display()))?;
    let segments = name.split('-').collect::<Vec<_>>();
    let valid_name = !segments.is_empty()
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        })
        && name
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase());
    if !valid_name {
        return Err(format!(
            "Claude projection {} has invalid agent name `{name}`",
            path.display()
        ));
    }
    Ok(())
}
