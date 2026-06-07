//! Read-loop memory loading for ASP-owned search pipe requests.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde_json::Value;

pub(super) fn read_loop_memory_selectors(cache_home: &Path, project_root: &Path) -> Vec<String> {
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
    selectors
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
