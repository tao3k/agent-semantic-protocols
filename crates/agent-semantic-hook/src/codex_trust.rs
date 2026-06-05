//! Codex user trust-state merge and cleanup helpers.

use std::path::Path;

const TRUST_BLOCK_BEGIN_PREFIX: &str = "# BEGIN agent-semantic-protocol trusted hook state: ";
pub(crate) const TRUST_BLOCK_END: &str = "# END agent-semantic-protocol trusted hook state";
const LEGACY_TRUST_BLOCK_BEGIN_PREFIX: &str = "# BEGIN semantic-agent-hook trusted hook state: ";
const LEGACY_TRUST_BLOCK_END: &str = "# END semantic-agent-hook trusted hook state";
const LEGACY_TYPO_TRUST_BLOCK_BEGIN_PREFIX: &str =
    "# BEGIN semantic-agent-protocol trusted hook state: ";
const LEGACY_TYPO_TRUST_BLOCK_END: &str = "# END semantic-agent-protocol trusted hook state";

pub(crate) fn codex_trust_block_begin(config_source_path: &Path) -> String {
    format!("{TRUST_BLOCK_BEGIN_PREFIX}{}", config_source_path.display())
}

pub(crate) fn merge_codex_trust_config(
    existing: &str,
    config_source_path: &Path,
    block: &str,
) -> String {
    let content = remove_stale_codex_trust_state_blocks(existing);
    let content = remove_legacy_codex_trust_state(&content, config_source_path);
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

fn remove_stale_codex_trust_state_blocks(existing: &str) -> String {
    let lines = existing.lines().collect::<Vec<_>>();
    let mut kept = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if let Some(path) = codex_trust_block_path(line) {
            if !Path::new(path).is_file() {
                if let Some(relative_end) = lines[index + 1..]
                    .iter()
                    .position(|candidate| is_codex_trust_block_end(candidate))
                {
                    index += relative_end + 2;
                } else {
                    index += 1;
                }
                continue;
            }
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
        LEGACY_TRUST_BLOCK_BEGIN_PREFIX,
        LEGACY_TYPO_TRUST_BLOCK_BEGIN_PREFIX,
    ]
    .iter()
    .find_map(|prefix| trimmed.strip_prefix(prefix).map(str::trim))
}

fn is_codex_trust_block_end(line: &str) -> bool {
    matches!(
        line.trim(),
        TRUST_BLOCK_END | LEGACY_TRUST_BLOCK_END | LEGACY_TYPO_TRUST_BLOCK_END
    )
}

fn remove_legacy_codex_trust_state(existing: &str, config_source_path: &Path) -> String {
    let legacy_begin = legacy_codex_trust_block_begin(config_source_path);
    let content = remove_managed_block(existing, &legacy_begin, LEGACY_TRUST_BLOCK_END);
    content
        .lines()
        .filter(|line| line.trim() != legacy_begin)
        .collect::<Vec<_>>()
        .join("\n")
}

fn legacy_codex_trust_block_begin(config_source_path: &Path) -> String {
    format!(
        "{LEGACY_TRUST_BLOCK_BEGIN_PREFIX}{}",
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
        let Some(relative_end) = content[start..].find(end) else {
            break;
        };
        let end_index = start + relative_end + end.len();
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
