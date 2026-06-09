//! Read-loop memory loading for ASP-owned search pipe requests.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub(super) fn read_loop_memory_selectors(
    cache_home: &Path,
    project_root: &Path,
    locator_root: &Path,
    scopes: &[PathBuf],
) -> Vec<String> {
    let path = cache_home
        .join("agent-semantic-protocol")
        .join("read-loop-memory.json");
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(memory) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    if !memory_project_matches(&memory, project_root) {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    let mut selectors = Vec::new();
    append_selector_array(&memory["seenSelectors"], &mut selectors, &mut seen);
    if let Some(entries) = memory["entries"].as_array() {
        for entry in entries {
            if let Some(selector) = entry["selector"].as_str() {
                push_unique_selector(selector, &mut selectors, &mut seen);
            }
        }
    }
    selector_scope_variants(selectors, project_root, locator_root, scopes)
}

fn memory_project_matches(memory: &Value, project_root: &Path) -> bool {
    let Some(project_root_value) = memory["projectRoot"].as_str() else {
        return true;
    };
    let memory_path = Path::new(project_root_value);
    if memory_path == project_root {
        return true;
    }
    let Ok(canonical_project_root) = project_root.canonicalize() else {
        return false;
    };
    memory_path
        .canonicalize()
        .is_ok_and(|canonical_memory_path| canonical_memory_path == canonical_project_root)
}

fn append_selector_array(value: &Value, selectors: &mut Vec<String>, seen: &mut HashSet<String>) {
    let Some(values) = value.as_array() else {
        return;
    };
    for value in values {
        if let Some(selector) = value.as_str() {
            push_unique_selector(selector, selectors, seen);
        }
    }
}

fn push_unique_selector(selector: &str, selectors: &mut Vec<String>, seen: &mut HashSet<String>) {
    let selector = selector.trim();
    if selector.is_empty() || !seen.insert(selector.to_string()) {
        return;
    }
    selectors.push(selector.to_string());
}

fn selector_scope_variants(
    selectors: Vec<String>,
    project_root: &Path,
    locator_root: &Path,
    scopes: &[PathBuf],
) -> Vec<String> {
    let scope_prefixes = scopes
        .iter()
        .flat_map(|scope| selector_scope_prefixes(project_root, locator_root, scope))
        .collect::<Vec<_>>();
    if scope_prefixes.is_empty() {
        return selectors;
    }
    let mut seen = HashSet::new();
    let mut variants = Vec::new();
    for selector in selectors {
        push_unique_selector(&selector, &mut variants, &mut seen);
        let Some((path, start, end)) = split_selector(&selector) else {
            continue;
        };
        for prefix in &scope_prefixes {
            if let Some(relative_path) = strip_scope_prefix(path, prefix) {
                push_unique_selector(
                    &format!("{relative_path}:{start}:{end}"),
                    &mut variants,
                    &mut seen,
                );
            }
        }
    }
    variants
}

fn selector_scope_prefixes(project_root: &Path, locator_root: &Path, scope: &Path) -> Vec<String> {
    let absolute = if scope.is_absolute() {
        scope.to_path_buf()
    } else {
        project_root.join(scope)
    };
    let mut prefixes = Vec::new();
    push_scope_prefix(&slash_path(&absolute), &mut prefixes);
    if let Ok(relative_to_locator) = absolute.strip_prefix(locator_root) {
        push_scope_prefix(&slash_path(relative_to_locator), &mut prefixes);
    }
    if let Ok(relative_to_project) = absolute.strip_prefix(project_root) {
        push_scope_prefix(&slash_path(relative_to_project), &mut prefixes);
    }
    prefixes
}

fn push_scope_prefix(prefix: &str, prefixes: &mut Vec<String>) {
    let prefix = prefix.trim_matches('/');
    if prefix.is_empty() || prefix == "." || prefixes.iter().any(|item| item == prefix) {
        return;
    }
    prefixes.push(prefix.to_string());
}

fn strip_scope_prefix<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    let path = path.trim_start_matches("./");
    let prefix = prefix.trim_start_matches("./");
    path.strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('/'))
        .filter(|rest| !rest.is_empty())
}

fn split_selector(selector: &str) -> Option<(&str, &str, &str)> {
    let mut parts = selector.rsplitn(3, ':');
    let end = parts.next()?;
    let start = parts.next()?;
    let path = parts.next()?;
    (!path.is_empty() && !start.is_empty() && !end.is_empty()).then_some((path, start, end))
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
