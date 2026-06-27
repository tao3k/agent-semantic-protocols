//! ASP-owned exact direct source reads for hook recovery.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAX_DIRECT_SOURCE_READ_LINES: usize = 24;

pub(super) fn is_asp_fast_direct_source_read(args: &[String]) -> bool {
    if !matches!(args.first().map(String::as_str), Some("query"))
        || arg_value(args, "--selector").is_none()
        || !args.iter().any(|arg| arg == "--code")
        || args.iter().any(|arg| arg == "--json")
        || has_any_flag(args, &["--term", "--treesitter-query", "--names-only"])
    {
        return false;
    }
    arg_value(args, "--from-hook") == Some("direct-source-read")
}

pub(super) fn run_asp_fast_direct_source_read_command(
    args: &[String],
    project_root: &Path,
    locator_root: &Path,
) -> Result<(), String> {
    let fallback_reason = arg_value(args, "--fallback-reason")
        .ok_or_else(|| {
            "direct-source-read requires --fallback-reason <reason>; use structural search or syntax-outline first".to_string()
        })?
        .trim();
    if fallback_reason.is_empty() {
        return Err("direct-source-read --fallback-reason must not be empty".to_string());
    }
    let selector = arg_value(args, "--selector")
        .ok_or_else(|| "direct-source-read requires --selector <path-or-range>".to_string())?;
    run_bounded_selector_read(
        selector,
        project_root,
        locator_root,
        MAX_DIRECT_SOURCE_READ_LINES,
        "direct-source-read",
    )
}

fn run_bounded_selector_read(
    selector: &str,
    project_root: &Path,
    locator_root: &Path,
    max_lines: usize,
    label: &str,
) -> Result<(), String> {
    let selector = parse_selector(selector)?;
    let (start, end) = selector.range.ok_or_else(|| {
        "direct-source-read requires a bounded selector range; whole-file fallback is disabled"
            .to_string()
    })?;
    let span = end - start + 1;
    if span > max_lines {
        return Err(format!(
            "{label} range {span} lines exceeds max {max_lines}; use skeleton or syntax-outline before exact code"
        ));
    }
    let path = resolve_selector_path(project_root, locator_root, &selector.path)?;
    let bytes =
        fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let output = select_line_range(&bytes, start, end);
    io::stdout()
        .write_all(&output)
        .map_err(|error| format!("failed to write direct-source-read stdout: {error}"))
}

#[derive(Debug, Eq, PartialEq)]
struct DirectReadSelector {
    path: PathBuf,
    range: Option<(usize, usize)>,
}

fn parse_selector(selector: &str) -> Result<DirectReadSelector, String> {
    if let Some((path, start, end)) = parse_colon_range_selector(selector) {
        return validate_selector(path, Some((start, end)));
    }
    if let Some((path, start, end)) = parse_dash_range_selector(selector) {
        return validate_selector(path, Some((start, end)));
    }
    validate_selector(PathBuf::from(selector), None)
}

fn parse_colon_range_selector(selector: &str) -> Option<(PathBuf, usize, usize)> {
    let (path_or_start, end_text) = selector.rsplit_once(':')?;
    let end = end_text.parse::<usize>().ok()?;
    let Some((path, start_text)) = path_or_start.rsplit_once(':') else {
        return Some((PathBuf::from(path_or_start), end, end));
    };
    let start = start_text.parse::<usize>().ok()?;
    Some((PathBuf::from(path), start, end))
}

fn parse_dash_range_selector(selector: &str) -> Option<(PathBuf, usize, usize)> {
    let (path, range_text) = selector.rsplit_once(':')?;
    let (start_text, end_text) = range_text.split_once('-')?;
    let start = start_text.parse::<usize>().ok()?;
    let end = end_text.parse::<usize>().ok()?;
    Some((PathBuf::from(path), start, end))
}

fn validate_selector(
    path: PathBuf,
    range: Option<(usize, usize)>,
) -> Result<DirectReadSelector, String> {
    if path.as_os_str().is_empty() {
        return Err("direct-source-read selector path is empty".to_string());
    }
    if let Some((start, end)) = range
        && (start == 0 || end < start)
    {
        return Err(format!(
            "invalid direct-source-read selector range {start}-{end}"
        ));
    }
    Ok(DirectReadSelector { path, range })
}

fn resolve_selector_path(
    project_root: &Path,
    locator_root: &Path,
    selector_path: &Path,
) -> Result<PathBuf, String> {
    let candidates = if selector_path.is_absolute() {
        vec![selector_path.to_path_buf()]
    } else {
        vec![
            locator_root.join(selector_path),
            project_root.join(selector_path),
        ]
    };
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            format!(
                "direct-source-read selector not found: {}",
                selector_path.display()
            )
        })
}

fn select_line_range(bytes: &[u8], start: usize, end: usize) -> Vec<u8> {
    let first_line = start.max(1);
    if end < first_line {
        return Vec::new();
    }
    bytes
        .split_inclusive(|byte| *byte == b'\n')
        .skip(first_line - 1)
        .take(end - first_line + 1)
        .flat_map(|line| line.iter().copied())
        .collect()
}

fn arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then_some(window[1].as_str()))
}

fn has_any_flag(args: &[String], flags: &[&str]) -> bool {
    args.iter().any(|arg| flags.contains(&arg.as_str()))
}
