//! Cargo workspace member and exclude discovery for source-index scope.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SourceIndexCargoWorkspace {
    pub(super) members: BTreeSet<String>,
    pub(super) excludes: BTreeSet<String>,
}

pub(super) fn source_index_cargo_workspace(
    project_root: &Path,
) -> Option<SourceIndexCargoWorkspace> {
    let text = fs::read_to_string(project_root.join("Cargo.toml")).ok()?;
    parsed_cargo_workspace(&text).or_else(|| fallback_cargo_workspace(&text))
}

fn parsed_cargo_workspace(text: &str) -> Option<SourceIndexCargoWorkspace> {
    let value = text.parse::<toml::Value>().ok()?;
    let workspace = value.get("workspace")?;
    let members = array_strings(workspace.get("members"));
    let excludes = array_strings(workspace.get("exclude"));
    if members.is_empty() && excludes.is_empty() {
        None
    } else {
        Some(SourceIndexCargoWorkspace { members, excludes })
    }
}

fn array_strings(value: Option<&toml::Value>) -> BTreeSet<String> {
    value
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .map(str::to_string)
        .collect()
}

fn fallback_cargo_workspace(text: &str) -> Option<SourceIndexCargoWorkspace> {
    let section = workspace_section(text)?;
    let members = fallback_array_strings(section, "members");
    let excludes = fallback_array_strings(section, "exclude");
    if members.is_empty() && excludes.is_empty() {
        None
    } else {
        Some(SourceIndexCargoWorkspace { members, excludes })
    }
}

fn workspace_section(text: &str) -> Option<&str> {
    let start = text.find("[workspace]")?;
    let section = &text[start + "[workspace]".len()..];
    let end = section
        .lines()
        .scan(0usize, |offset, line| {
            let current = *offset;
            *offset += line.len() + 1;
            Some((current, line))
        })
        .skip_while(|(_, line)| line.trim().is_empty())
        .find_map(|(offset, line)| {
            let trimmed = line.trim();
            (trimmed.starts_with('[') && trimmed != "[workspace]").then_some(offset)
        })
        .unwrap_or(section.len());
    Some(&section[..end])
}

fn fallback_array_strings(section: &str, key: &str) -> BTreeSet<String> {
    let mut capture = String::new();
    let mut active = false;
    for line in section.lines() {
        let trimmed = line.trim();
        if !active && trimmed.starts_with(key) && trimmed.contains('=') {
            active = true;
        }
        if active {
            capture.push_str(trimmed);
            capture.push('\n');
            if trimmed.contains(']') {
                break;
            }
        }
    }
    quoted_strings(&capture)
}

fn quoted_strings(text: &str) -> BTreeSet<String> {
    let mut values = BTreeSet::new();
    let mut in_string = false;
    let mut value = String::new();
    for character in text.chars() {
        if character == '"' {
            if in_string {
                values.insert(value.clone());
                value.clear();
            }
            in_string = !in_string;
        } else if in_string {
            value.push(character);
        }
    }
    values
}
