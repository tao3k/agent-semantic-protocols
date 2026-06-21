//! Graph-turbo node projection for search-pipe candidates and project topology.

use std::{collections::BTreeSet, fs, path::Path};

use serde_json::{Value, json};

use super::{
    search_language_files::language_file_spec,
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
    candidates: &[Candidate],
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

    let submodule_paths = project_submodule_paths(workspace_root);
    let language_projects = language_project_roots(workspace_root, language_id, candidates);
    for submodule_path in &submodule_paths {
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
    for project in language_projects {
        let project_value = format!("{language_id}:{}", project.root_path);
        let project_id = stable_node_id("language-project", &project_value);
        let primary_marker = project
            .project_markers
            .first()
            .map(|marker| marker.path.as_str());
        nodes.push(json!({
            "id": project_id.clone(),
            "kind": "language-project",
            "role": "project-root",
            "value": project.root_path,
            "action": "topology",
            "path": project.root_path,
            "confidence": "exact",
            "fields": {
                "languageId": language_id,
                "projectMarker": primary_marker,
            },
        }));
        edges.push(json!({
            "source": provider_id,
            "target": project_id,
            "relation": "has_language_project",
        }));

        for marker in &project.project_markers {
            let marker_value = format!("{language_id}:{}", marker.path);
            let marker_id = stable_node_id("project-marker", &marker_value);
            nodes.push(json!({
                "id": marker_id.clone(),
                "kind": "project-marker",
                "role": "project-marker",
                "value": marker.path.as_str(),
                "action": "topology",
                "path": marker.path.as_str(),
                "confidence": "exact",
                "fields": {
                    "languageId": language_id,
                    "marker": marker.name.as_str(),
                },
            }));
            edges.push(json!({
                "source": project_id,
                "target": marker_id,
                "relation": "declared_by",
            }));
        }

        for marker in &project.dependency_markers {
            let marker_value = format!("{language_id}:{}", marker.path);
            let marker_id = stable_node_id("dependency-marker", &marker_value);
            nodes.push(json!({
                "id": marker_id.clone(),
                "kind": "dependency-marker",
                "role": "dependency-source",
                "value": marker.path.as_str(),
                "action": "topology",
                "path": marker.path.as_str(),
                "confidence": "exact",
                "fields": {
                    "languageId": language_id,
                    "marker": marker.name.as_str(),
                },
            }));
            edges.push(json!({
                "source": project_id,
                "target": marker_id,
                "relation": "uses_dependency_marker",
            }));
        }

        for submodule_path in &submodule_paths {
            if path_is_under(&project.root_path, submodule_path) {
                edges.push(json!({
                    "source": stable_node_id("submodule", submodule_path),
                    "target": project_id,
                    "relation": "contains_project",
                }));
            }
        }
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

pub(super) fn project_submodule_paths(workspace_root: &Path) -> Vec<String> {
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

pub(super) fn path_is_under(path: &str, root: &str) -> bool {
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/'))
}

#[derive(Debug)]
struct LanguageProjectRoot {
    root_path: String,
    project_markers: Vec<TopologyMarker>,
    dependency_markers: Vec<TopologyMarker>,
}

#[derive(Debug)]
struct TopologyMarker {
    name: String,
    path: String,
}

fn language_project_roots(
    workspace_root: &Path,
    language_id: &str,
    candidates: &[Candidate],
) -> Vec<LanguageProjectRoot> {
    let file_spec = language_file_spec(language_id);
    if file_spec.project_markers().is_empty() || !workspace_root.exists() {
        return Vec::new();
    }
    let mut seen_roots = BTreeSet::new();
    let mut projects = Vec::new();
    push_language_project_root(
        &mut projects,
        &mut seen_roots,
        workspace_root,
        workspace_root,
        file_spec.project_markers(),
        file_spec.dependency_markers(),
    );
    for candidate in candidates {
        let candidate_path = workspace_root.join(&candidate.path);
        let start = if candidate_path.is_file() {
            candidate_path.parent().unwrap_or(workspace_root)
        } else {
            candidate_path.as_path()
        };
        for root in candidate_project_roots(workspace_root, start, file_spec.project_markers()) {
            push_language_project_root(
                &mut projects,
                &mut seen_roots,
                workspace_root,
                root,
                file_spec.project_markers(),
                file_spec.dependency_markers(),
            );
        }
    }
    projects.sort_by(|left, right| {
        left.root_path.cmp(&right.root_path).then_with(|| {
            left.project_markers
                .first()
                .map(|marker| marker.path.as_str())
                .cmp(
                    &right
                        .project_markers
                        .first()
                        .map(|marker| marker.path.as_str()),
                )
        })
    });
    projects
}

fn candidate_project_roots<'a>(
    workspace_root: &'a Path,
    start: &'a Path,
    config_filenames: &[String],
) -> Vec<&'a Path> {
    let mut roots = Vec::new();
    let mut current = Some(start);
    while let Some(path) = current {
        if !path.starts_with(workspace_root) {
            break;
        }
        if config_filenames
            .iter()
            .any(|config_filename| path.join(config_filename).is_file())
        {
            roots.push(path);
        }
        if path == workspace_root {
            break;
        }
        current = path.parent();
    }
    roots
}

fn push_language_project_root(
    projects: &mut Vec<LanguageProjectRoot>,
    seen_roots: &mut BTreeSet<String>,
    workspace_root: &Path,
    root: &Path,
    project_markers: &[String],
    dependency_markers: &[String],
) {
    let project_markers = topology_marker_paths(workspace_root, root, project_markers);
    if project_markers.is_empty() {
        return;
    }
    let dependency_markers = topology_marker_paths(workspace_root, root, dependency_markers);
    let root_path = relative_topology_path(workspace_root, root).unwrap_or_else(|| ".".to_string());
    if !seen_roots.insert(root_path.clone()) {
        return;
    }
    projects.push(LanguageProjectRoot {
        root_path,
        project_markers,
        dependency_markers,
    });
}

fn topology_marker_paths(
    workspace_root: &Path,
    root: &Path,
    marker_names: &[String],
) -> Vec<TopologyMarker> {
    marker_names
        .iter()
        .filter_map(|marker_name| {
            let marker_path = root.join(marker_name);
            let path = marker_path
                .is_file()
                .then(|| relative_topology_path(workspace_root, &marker_path))
                .flatten()?;
            Some(TopologyMarker {
                name: marker_name.clone(),
                path,
            })
        })
        .collect()
}

fn relative_topology_path(workspace_root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(workspace_root).ok()?;
    if relative.as_os_str().is_empty() {
        return Some(".".to_string());
    }
    Some(normalize_topology_path(relative))
}

fn normalize_topology_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
