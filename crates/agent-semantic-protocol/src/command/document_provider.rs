//! Document language provider facade backed by orgize.

use super::{org_archive, org_capture, org_recall, search_config::AspConfig};
use orgize::agent::{self, DocumentLanguage, DocumentWalkConfig};
use std::ffi::OsString;

const DOCUMENT_LANGUAGES: &[&str] = &["org", "md"];

pub(crate) fn is_document_language(language_id: &str) -> bool {
    DOCUMENT_LANGUAGES.contains(&language_id)
}

pub(crate) fn run_language_command(language_id: &str, args: &[String]) -> Result<(), String> {
    run_language_command_with_config(language_id, args, &AspConfig::default())
}

pub(crate) fn run_language_command_with_config(
    language_id: &str,
    args: &[String],
    config: &AspConfig,
) -> Result<(), String> {
    if is_language_help(args) {
        println!("{}", usage(language_id));
        return Ok(());
    }
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage(language_id));
    };
    if command == "contract" && language_id == "org" {
        return agent::run_org_contract_command(args[1..].to_vec());
    }
    if command == "archive" && language_id == "org" {
        return org_archive::run_org_archive_command(&args[1..]);
    }
    if command == "recall" && language_id == "org" {
        return org_recall::run_org_recall_command(&args[1..]);
    }
    if command == "capture" && language_id == "org" {
        let capture_args = &args[1..];
        if is_capture_state_command(capture_args) || capture_has_contract(capture_args) {
            return org_capture::run_org_capture_command(capture_args);
        }
        return Err(
            "asp org capture expects `--contract CONTRACT_ID`; use `asp org capture --contract agent.task.v1 --title TITLE --target-file ORG_FILE` for a contract-checked non-mutating Org entry. ASP Org state is initialized during install/sync."
                .to_string(),
        );
    }
    if is_document_command(command) {
        let _generic_session_env = GenericSessionEnvGuard::remove_for(language_id);
        let document_args = normalize_document_command_args(language_id, command, args)?;
        return agent::run_document_command_with_walk_config(
            document_language(language_id)?,
            document_args,
            DocumentWalkConfig::new(
                config.search.ignore_dirs.clone(),
                config.search.include_hidden_dirs.clone(),
            ),
        );
    }
    if language_id == "org" && is_embedded_org_command(command) {
        return agent::run_org_cli_command(args.to_vec());
    }
    if !is_document_command(command) {
        return Err(format!(
            "asp {language_id}: unsupported document command `{command}`; supported commands are {}",
            supported_commands(language_id)
        ));
    }

    unreachable!("document commands are returned above")
}

fn normalize_document_command_args(
    language_id: &str,
    command: &str,
    args: &[String],
) -> Result<Vec<String>, String> {
    if language_id == "org" && command == "query" {
        return normalize_org_query_item_selector_args(args);
    }
    Ok(args.to_vec())
}

fn normalize_org_query_item_selector_args(args: &[String]) -> Result<Vec<String>, String> {
    let Some(selector_index) = args.iter().position(|arg| arg == "--selector") else {
        return Ok(args.to_vec());
    };
    let Some(selector) = args.get(selector_index + 1) else {
        return Ok(args.to_vec());
    };
    let Some(range_selector) = org_item_heading_selector_to_line_range(selector)? else {
        return Ok(args.to_vec());
    };

    let mut normalized = args.to_vec();
    normalized[selector_index + 1] = range_selector;
    Ok(normalized)
}

fn org_item_heading_selector_to_line_range(selector: &str) -> Result<Option<String>, String> {
    let Some(rest) = selector.strip_prefix("org://") else {
        return Ok(None);
    };
    let Some((path, fragment)) = rest.split_once('#') else {
        return Ok(None);
    };
    let Some(slug) = fragment.strip_prefix("item/heading/") else {
        return Ok(None);
    };
    if path.is_empty() || slug.is_empty() {
        return Ok(None);
    }

    let source = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to resolve Org item selector `{selector}`: {err}"))?;
    let Some((start_line, end_line)) = org_heading_slug_line_range(&source, slug) else {
        return Ok(None);
    };
    Ok(Some(format!("{path}:{start_line}-{end_line}")))
}

fn org_heading_slug_line_range(source: &str, expected_slug: &str) -> Option<(usize, usize)> {
    let mut heading: Option<(usize, usize)> = None;
    let mut last_line = 0;

    for (index, line) in source.lines().enumerate() {
        let line_no = index + 1;
        last_line = line_no;
        let Some((level, title)) = org_heading_level_and_title(line) else {
            continue;
        };
        if let Some((start_line, start_level)) = heading {
            if level <= start_level {
                return Some((start_line, line_no.saturating_sub(1).max(start_line)));
            }
        }
        if org_heading_slug(title) == expected_slug {
            heading = Some((line_no, level));
        }
    }

    heading.map(|(start_line, _)| (start_line, last_line.max(start_line)))
}

fn org_heading_level_and_title(line: &str) -> Option<(usize, &str)> {
    let stars = line.chars().take_while(|ch| *ch == '*').count();
    if stars == 0 {
        return None;
    }
    let title = line.get(stars..)?.strip_prefix(' ')?;
    Some((stars, title.trim()))
}

fn org_heading_slug(title: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;
    for ch in title.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(ch);
            pending_dash = false;
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            pending_dash = true;
        }
    }
    slug
}

struct GenericSessionEnvGuard {
    values: Vec<(&'static str, Option<OsString>)>,
}

impl GenericSessionEnvGuard {
    fn remove_for(language_id: &str) -> Self {
        let names = if language_id == "org" {
            &["AGENT_SESSION_ID", "SESSION_ID"][..]
        } else {
            &[][..]
        };
        let values = names
            .iter()
            .map(|name| {
                let value = std::env::var_os(name);
                unsafe {
                    std::env::remove_var(name);
                }
                (*name, value)
            })
            .collect();
        Self { values }
    }
}

impl Drop for GenericSessionEnvGuard {
    fn drop(&mut self) {
        for (name, value) in self.values.drain(..) {
            if let Some(value) = value {
                unsafe {
                    std::env::set_var(name, value);
                }
            } else {
                unsafe {
                    std::env::remove_var(name);
                }
            }
        }
    }
}

fn is_language_help(args: &[String]) -> bool {
    args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h" | "help")
}

fn is_capture_state_command(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("init" | "--state-root" | "--source-dir" | "--help" | "-h" | "help")
    )
}

fn capture_has_contract(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--contract")
}

fn document_language(language_id: &str) -> Result<DocumentLanguage, String> {
    match language_id {
        "org" => Ok(DocumentLanguage::Org),
        "md" => Ok(DocumentLanguage::Markdown),
        _ => Err(format!("unsupported document language `{language_id}`")),
    }
}

fn usage(language_id: &str) -> String {
    format!(
        "usage: asp {language_id} <{}> ...",
        supported_commands(language_id)
    )
}

fn supported_commands(language_id: &str) -> &'static str {
    match language_id {
        "org" => {
            "guide|search|query|elements-query|contract|capture|recall|archive|export|fmt|lint"
        }
        _ => "guide|search|query|elements-query",
    }
}

fn is_document_command(command: &str) -> bool {
    matches!(command, "guide" | "search" | "query" | "elements-query")
}

fn is_embedded_org_command(command: &str) -> bool {
    matches!(command, "export" | "fmt" | "lint")
}
