//! Codex user trust-state merge and cleanup helpers.

use std::path::Path;

const TRUST_BLOCK_BEGIN_PREFIX: &str = "# BEGIN agent-semantic-protocol trusted hook state: ";
pub(crate) const TRUST_BLOCK_END: &str = "# END agent-semantic-protocol trusted hook state";
const RETIRED_TRUST_BLOCK_BEGIN_PREFIX: &str = "# BEGIN semantic-agent-hook trusted hook state: ";
const RETIRED_TRUST_BLOCK_END: &str = "# END semantic-agent-hook trusted hook state";
const RETIRED_TYPO_TRUST_BLOCK_BEGIN_PREFIX: &str =
    "# BEGIN semantic-agent-protocol trusted hook state: ";
const RETIRED_TYPO_TRUST_BLOCK_END: &str = "# END semantic-agent-protocol trusted hook state";

pub(crate) fn codex_trust_block_begin(config_source_path: &Path) -> String {
    format!("{TRUST_BLOCK_BEGIN_PREFIX}{}", config_source_path.display())
}

pub(crate) fn merge_codex_trust_config(
    existing: &str,
    config_source_path: &Path,
    block: &str,
) -> String {
    let content = remove_stale_codex_trust_state_blocks(existing);
    let content = remove_retired_codex_trust_state(&content, config_source_path);
    let content = remove_managed_block(
        &content,
        &codex_trust_block_begin(config_source_path),
        TRUST_BLOCK_END,
    );
    let content = remove_hook_state_entries_for_config(&content, config_source_path);
    let prefix = content.trim();
    if prefix.is_empty() {
        format!("{}\n", block.trim_end())
    } else {
        format!("{}\n\n{}\n", prefix, block.trim_end())
    }
}

pub(crate) fn codex_project_trusted(config: &toml::Value, project_root: &Path) -> bool {
    config
        .get("projects")
        .and_then(toml::Value::as_table)
        .and_then(|projects| projects.get(&project_root.display().to_string()))
        .and_then(toml::Value::as_table)
        .and_then(|project| project.get("trust_level"))
        .and_then(toml::Value::as_str)
        == Some("trusted")
}

pub(crate) fn merge_codex_project_trust_config(
    existing: &str,
    project_root: &Path,
) -> Result<String, String> {
    let parsed = parse_codex_config_or_empty(existing)?;
    if codex_project_trusted(&parsed, project_root) {
        return Ok(existing.to_string());
    }
    let entry_exists = codex_project_entry_exists(&parsed, project_root);
    if (!entry_exists || codex_project_table_header_exists(existing, project_root))
        && let Some(merged) = merge_codex_project_trust_config_lines(existing, project_root)
        && toml::from_str::<toml::Value>(&merged).is_ok()
    {
        return Ok(merged);
    }
    merge_codex_project_trust_config_structured(parsed, project_root)
}

fn parse_codex_config_or_empty(existing: &str) -> Result<toml::Value, String> {
    if existing.trim().is_empty() {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    toml::from_str(existing).map_err(|error| error.to_string())
}

fn codex_project_entry_exists(config: &toml::Value, project_root: &Path) -> bool {
    config
        .get("projects")
        .and_then(toml::Value::as_table)
        .is_some_and(|projects| projects.contains_key(&project_root.display().to_string()))
}

fn merge_codex_project_trust_config_lines(existing: &str, project_root: &Path) -> Option<String> {
    let header = codex_project_trust_header(project_root);
    let mut lines = existing.lines().map(str::to_string).collect::<Vec<_>>();
    let Some(start) = lines.iter().position(|line| {
        toml_table_header(line.trim())
            .as_deref()
            .is_some_and(|candidate| candidate == header)
    }) else {
        if !lines.is_empty() && lines.last().is_some_and(|line| !line.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push(format!("[{header}]"));
        lines.push("trust_level = \"trusted\"".to_string());
        return Some(format!("{}\n", lines.join("\n")));
    };
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, line)| toml_table_header(line.trim()).map(|_| index))
        .unwrap_or(lines.len());
    if let Some(index) = lines[start + 1..end]
        .iter()
        .position(|line| toml_key(line.trim()) == Some("trust_level"))
        .map(|relative_index| start + 1 + relative_index)
    {
        lines[index] = "trust_level = \"trusted\"".to_string();
    } else {
        lines.insert(end, "trust_level = \"trusted\"".to_string());
    }
    Some(format!("{}\n", lines.join("\n")))
}

fn merge_codex_project_trust_config_structured(
    mut parsed: toml::Value,
    project_root: &Path,
) -> Result<String, String> {
    let root = parsed
        .as_table_mut()
        .ok_or_else(|| "Codex user config TOML root must be a table".to_string())?;
    let projects = root
        .entry("projects".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let projects = projects
        .as_table_mut()
        .ok_or_else(|| "Codex user config projects must be a table".to_string())?;
    let project = projects
        .entry(project_root.display().to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let project = project
        .as_table_mut()
        .ok_or_else(|| "Codex user config project trust entry must be a table".to_string())?;
    project.insert(
        "trust_level".to_string(),
        toml::Value::String("trusted".to_string()),
    );
    toml::to_string_pretty(&parsed)
        .map(|content| {
            if content.ends_with('\n') {
                content
            } else {
                format!("{content}\n")
            }
        })
        .map_err(|error| error.to_string())
}

fn codex_project_table_header_exists(existing: &str, project_root: &Path) -> bool {
    let header = codex_project_trust_header(project_root);
    existing.lines().any(|line| {
        toml_table_header(line.trim())
            .as_deref()
            .is_some_and(|candidate| candidate == header)
    })
}

fn codex_project_trust_header(project_root: &Path) -> String {
    format!(
        "projects.{}",
        toml_basic_string(&project_root.display().to_string())
    )
}

fn toml_key(trimmed: &str) -> Option<&str> {
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    trimmed.split_once('=').map(|(key, _)| key.trim())
}

fn toml_table_header(trimmed: &str) -> Option<String> {
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return None;
    }
    let header = trimmed.trim_matches(['[', ']']).trim();
    (!header.is_empty()).then(|| header.to_string())
}

fn remove_stale_codex_trust_state_blocks(existing: &str) -> String {
    let lines = existing.lines().collect::<Vec<_>>();
    let mut kept = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if let Some(path) = codex_trust_block_path(line)
            && !Path::new(path).is_file()
        {
            if let Some(end_marker_index) = lines[index + 1..]
                .iter()
                .position(|candidate| is_codex_trust_block_end(candidate))
            {
                index += end_marker_index + 2;
            } else {
                index += 1;
            }
            continue;
        }
        kept.push(line);
        index += 1;
    }
    kept.join("\n")
}

fn codex_trust_block_path(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    [
        TRUST_BLOCK_BEGIN_PREFIX,
        RETIRED_TRUST_BLOCK_BEGIN_PREFIX,
        RETIRED_TYPO_TRUST_BLOCK_BEGIN_PREFIX,
    ]
    .iter()
    .find_map(|prefix| trimmed.strip_prefix(prefix).map(str::trim))
}

fn is_codex_trust_block_end(line: &str) -> bool {
    matches!(
        line.trim(),
        TRUST_BLOCK_END | RETIRED_TRUST_BLOCK_END | RETIRED_TYPO_TRUST_BLOCK_END
    )
}

fn remove_retired_codex_trust_state(existing: &str, config_source_path: &Path) -> String {
    let retired_begin = retired_codex_trust_block_begin(config_source_path);
    let content = remove_managed_block(existing, &retired_begin, RETIRED_TRUST_BLOCK_END);
    content
        .lines()
        .filter(|line| line.trim() != retired_begin)
        .collect::<Vec<_>>()
        .join("\n")
}

fn retired_codex_trust_block_begin(config_source_path: &Path) -> String {
    format!(
        "{RETIRED_TRUST_BLOCK_BEGIN_PREFIX}{}",
        config_source_path.display()
    )
}

fn remove_hook_state_entries_for_config(existing: &str, config_source_path: &Path) -> String {
    let escaped_path = toml_basic_string(&config_source_path.display().to_string());
    let state_prefix = format!(
        "[hooks.state.{}:",
        escaped_path
            .strip_suffix('"')
            .expect("basic string ends with quote")
    );
    existing
        .lines()
        .scan(false, |skipping, line| {
            let trimmed = line.trim();
            if trimmed.starts_with("[hooks.state.") && trimmed.starts_with(&state_prefix) {
                *skipping = true;
                return Some(None);
            }
            if *skipping && trimmed.starts_with('[') {
                *skipping = false;
            }
            Some((!*skipping).then_some(line))
        })
        .flatten()
        .collect::<Vec<_>>()
        .join("\n")
}

fn remove_managed_block(existing: &str, begin: &str, end: &str) -> String {
    let mut content = existing.to_string();
    while let Some(start) = content.find(begin) {
        let Some(end_marker_start) = content[start..].find(end) else {
            break;
        };
        let end_index = start + end_marker_start + end.len();
        content.replace_range(start..end_index, "");
    }
    content.trim().to_string()
}

pub(crate) fn toml_basic_string(value: &str) -> String {
    let mut output = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if c.is_control() => output.push_str(&format!("\\u{:04X}", c as u32)),
            c => output.push(c),
        }
    }
    output.push('"');
    output
}
