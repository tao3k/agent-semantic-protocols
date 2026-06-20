//! Graph-turbo node projection for search-pipe candidates and project topology.

use std::{collections::BTreeSet, fs, path::Path};

use serde_json::{Value, json};

use super::{
    search_pipe_model::Candidate,
    search_pipe_projection::{
        candidate_end_line, candidate_selector, graph_projection_action, is_document_language,
    },
};

const HOT_CONTEXT_BEFORE_LINES: usize = 8;
const HOT_CONTEXT_AFTER_LINES: usize = 12;

pub(super) fn append_project_topology_nodes(
    nodes: &mut Vec<Value>,
    edges: &mut Vec<Value>,
    language_id: &str,
    workspace_root: &Path,
) {
    let workspace_id = stable_node_id("workspace", ".");
    nodes.push(json!({
        "id": workspace_id.clone(),
        "kind": "workspace",
        "role": "root",
        "value": ".",
        "action": "topology",
        "path": ".",
        "confidence": "exact",
    }));

    let provider_value = format!("{language_id}:.");
    let provider_id = stable_node_id("provider-root", &provider_value);
    nodes.push(json!({
        "id": provider_id.clone(),
        "kind": "provider-root",
        "role": "language-root",
        "value": provider_value,
        "action": "topology",
        "path": ".",
        "confidence": "exact",
        "fields": {
            "languageId": language_id,
        },
    }));
    edges.push(json!({
        "source": workspace_id,
        "target": provider_id,
        "relation": "has_provider_root",
    }));

    for submodule_path in project_submodule_paths(workspace_root) {
        let submodule_id = stable_node_id("submodule", &submodule_path);
        nodes.push(json!({
            "id": submodule_id.clone(),
            "kind": "submodule",
            "role": "workspace-member",
            "value": submodule_path.clone(),
            "action": "topology",
            "path": submodule_path,
            "confidence": "exact",
            "fields": {
                "declaredBy": ".gitmodules",
            },
        }));
        edges.push(json!({
            "source": workspace_id,
            "target": submodule_id,
            "relation": "has_submodule",
        }));
    }
}

pub(super) fn append_submodule_owner_edges(
    edges: &mut Vec<Value>,
    workspace_root: &Path,
    owners: &[String],
) {
    let submodule_paths = project_submodule_paths(workspace_root);
    if submodule_paths.is_empty() {
        return;
    }
    let mut seen = BTreeSet::new();
    for owner in owners {
        let Some(submodule_path) = submodule_paths
            .iter()
            .find(|submodule_path| path_is_under(owner, submodule_path))
        else {
            continue;
        };
        let key = format!("{submodule_path}:{owner}");
        if seen.insert(key) {
            edges.push(json!({
                "source": stable_node_id("submodule", submodule_path),
                "target": stable_node_id("owner", owner),
                "relation": "contains",
            }));
        }
    }
}

pub(super) fn append_candidate_nodes(
    nodes: &mut Vec<Value>,
    language_id: &str,
    candidates: &[Candidate],
    limit: usize,
) {
    for candidate in candidates.iter().take(limit) {
        nodes.push(json!({
            "id": candidate_node_id(candidate),
            "kind": "item",
            "role": "symbol",
            "value": candidate.symbol,
            "action": "syntax",
            "path": candidate.path,
            "ownerPath": candidate.path,
            "symbol": candidate.symbol,
            "startLine": candidate.line,
            "endLine": candidate_end_line(candidate),
            "locator": candidate_selector(language_id, candidate),
            "matchText": candidate.text,
            "syntaxQuery": candidate_tree_sitter_pattern(language_id, &candidate.symbol),
            "source": candidate.source,
            "confidence": candidate.confidence,
        }));
    }
}

pub(super) fn append_hot_nodes(
    nodes: &mut Vec<Value>,
    language_id: &str,
    candidates: &[Candidate],
    limit: usize,
) {
    for candidate in candidates.iter().take(limit) {
        let document = is_document_language(language_id);
        let (start_line, end_line) = if document {
            (candidate.line, candidate_end_line(candidate))
        } else {
            hot_context_range(candidate.line)
        };
        let locator = if document {
            candidate_selector(language_id, candidate)
        } else {
            format!("{}:{}:{end_line}", candidate.path, start_line)
        };
        nodes.push(json!({
            "id": hot_node_id(candidate),
            "kind": "hot",
            "role": "range",
            "value": candidate.symbol,
            "action": graph_projection_action(language_id),
            "path": candidate.path,
            "ownerPath": candidate.path,
            "symbol": candidate.symbol,
            "startLine": start_line,
            "endLine": end_line,
            "locator": locator,
            "matchText": candidate.text,
            "source": candidate.source,
            "confidence": candidate.confidence,
        }));
    }
}

pub(super) fn candidate_node_id(candidate: &Candidate) -> String {
    stable_node_id(
        "item",
        &format!(
            "{}:{}:{}-{}",
            candidate.path,
            candidate.symbol,
            candidate.line,
            candidate_end_line(candidate)
        ),
    )
}

pub(super) fn hot_node_id(candidate: &Candidate) -> String {
    stable_node_id(
        "hot",
        &format!(
            "{}:{}:{}-{}",
            candidate.path,
            candidate.symbol,
            candidate.line,
            candidate_end_line(candidate)
        ),
    )
}

pub(super) fn stable_node_id(kind: &str, value: &str) -> String {
    let mut rendered = String::with_capacity(kind.len() + value.len() + 1);
    rendered.push_str(kind);
    rendered.push(':');
    for character in value.chars() {
        if character == '_' || character == '-' || character == '/' || character == '.' {
            rendered.push(character);
        } else if character.is_ascii_alphanumeric() {
            rendered.push(character.to_ascii_lowercase());
        } else {
            rendered.push('-');
        }
    }
    while rendered.ends_with('-') {
        rendered.pop();
    }
    if rendered.len() == kind.len() + 1 {
        rendered.push_str("node");
    }
    rendered
}

fn hot_context_range(line: usize) -> (usize, usize) {
    (
        line.saturating_sub(HOT_CONTEXT_BEFORE_LINES).max(1),
        line + HOT_CONTEXT_AFTER_LINES,
    )
}

fn candidate_tree_sitter_pattern(language_id: &str, symbol: &str) -> Option<String> {
    let escaped_symbol = symbol.replace('\\', "\\\\").replace('"', "\\\"");
    match language_id {
        "rust" => Some(format!(
            "((function_item name: (_) @function.name) (#eq? @function.name \"{escaped_symbol}\"))"
        )),
        "python" => Some(format!(
            "((function_definition name: (identifier) @function.name) (#eq? @function.name \"{escaped_symbol}\"))"
        )),
        _ => None,
    }
}

fn project_submodule_paths(workspace_root: &Path) -> Vec<String> {
    let Ok(content) = fs::read_to_string(workspace_root.join(".gitmodules")) else {
        return Vec::new();
    };
    let mut paths = BTreeSet::new();
    for line in content.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("path") else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        let path = rest.trim().trim_matches('"').replace('\\', "/");
        if !path.is_empty() && !path.starts_with('/') {
            paths.insert(path);
        }
    }
    paths.into_iter().collect()
}

fn path_is_under(path: &str, root: &str) -> bool {
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/'))
}
