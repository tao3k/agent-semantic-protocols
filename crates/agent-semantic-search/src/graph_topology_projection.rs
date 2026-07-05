use std::{collections::BTreeSet, fs, path::Path};

use serde_json::{Value, json};

use crate::{GraphProjectionCandidate, language_file_spec, stable_graph_node_id};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GraphTopologyProjection {
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
}

pub struct GraphTopologyProjectionRequest<'a> {
    language_id: &'a str,
    workspace_root: &'a Path,
    candidates: &'a [GraphProjectionCandidate],
}

impl<'a> GraphTopologyProjectionRequest<'a> {
    pub fn new(
        language_id: &'a str,
        workspace_root: &'a Path,
        candidates: &'a [GraphProjectionCandidate],
    ) -> Self {
        Self {
            language_id,
            workspace_root,
            candidates,
        }
    }
}

impl<'a> From<(&'a str, &'a Path, &'a [GraphProjectionCandidate])>
    for GraphTopologyProjectionRequest<'a>
{
    fn from(
        (language_id, workspace_root, candidates): (
            &'a str,
            &'a Path,
            &'a [GraphProjectionCandidate],
        ),
    ) -> Self {
        Self {
            language_id,
            workspace_root,
            candidates,
        }
    }
}

pub fn graph_project_topology_projection(
    request: GraphTopologyProjectionRequest<'_>,
) -> GraphTopologyProjection {
    let mut projection = GraphTopologyProjection::default();
    let language_id = request.language_id;
    let workspace_root = request.workspace_root;
    let candidates = request.candidates;
    let workspace_id = stable_graph_node_id("workspace", ".");
    projection.nodes.push(json!({
        "id": workspace_id.clone(),
        "kind": "workspace",
        "role": "root",
        "value": ".",
        "action": "topology",
        "path": ".",
        "confidence": "exact",
    }));

    let provider_value = format!("{language_id}:.");
    let provider_id = stable_graph_node_id("provider-root", &provider_value);
    projection.nodes.push(json!({
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
    projection.edges.push(json!({
        "source": workspace_id,
        "target": provider_id,
        "relation": "has_provider_root",
    }));

    let submodule_paths = graph_project_submodule_paths(workspace_root);
    let language_projects = language_project_roots(workspace_root, language_id, candidates);
    for submodule_path in &submodule_paths {
        let submodule_id = stable_graph_node_id("submodule", submodule_path);
        projection.nodes.push(json!({
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
        projection.edges.push(json!({
            "source": workspace_id,
            "target": submodule_id,
            "relation": "has_submodule",
        }));
    }

    for project in language_projects {
        let project_value = format!("{language_id}:{}", project.root_path);
        let project_id = stable_graph_node_id("language-project", &project_value);
        let primary_marker = project
            .project_markers
            .first()
            .map(|marker| marker.path.as_str());
        projection.nodes.push(json!({
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
        projection.edges.push(json!({
            "source": provider_id,
            "target": project_id,
            "relation": "has_language_project",
        }));

        for marker in &project.project_markers {
            let marker_value = format!("{language_id}:{}", marker.path);
            let marker_id = stable_graph_node_id("project-marker", &marker_value);
            projection.nodes.push(json!({
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
            projection.edges.push(json!({
                "source": project_id,
                "target": marker_id,
                "relation": "declared_by",
            }));
        }

        for marker in &project.dependency_markers {
            let marker_value = format!("{language_id}:{}", marker.path);
            let marker_id = stable_graph_node_id("dependency-marker", &marker_value);
            projection.nodes.push(json!({
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
            projection.edges.push(json!({
                "source": project_id,
                "target": marker_id,
                "relation": "uses_dependency_marker",
            }));
        }

        for submodule_path in &submodule_paths {
            if graph_path_is_under(&project.root_path, submodule_path) {
                projection.edges.push(json!({
                    "source": stable_graph_node_id("submodule", submodule_path),
                    "target": project_id,
                    "relation": "contains_project",
                }));
            }
        }
    }
    projection
}

pub fn graph_submodule_owner_edges(workspace_root: &Path, owners: &[String]) -> Vec<Value> {
    let submodule_paths = graph_project_submodule_paths(workspace_root);
    if submodule_paths.is_empty() {
        return Vec::new();
    }
    let mut seen = BTreeSet::new();
    let mut edges = Vec::new();
    for owner in owners {
        let Some(submodule_path) = submodule_paths
            .iter()
            .find(|submodule_path| graph_path_is_under(owner, submodule_path))
        else {
            continue;
        };
        let key = format!("{submodule_path}:{owner}");
        if seen.insert(key) {
            edges.push(json!({
                "source": stable_graph_node_id("submodule", submodule_path),
                "target": stable_graph_node_id("owner", owner),
                "relation": "contains",
            }));
        }
    }
    edges
}

pub fn graph_project_submodule_paths(workspace_root: &Path) -> Vec<String> {
    graph_project_submodule_paths_from_content(
        &fs::read_to_string(workspace_root.join(".gitmodules")).unwrap_or_default(),
    )
}

pub fn graph_project_submodule_paths_from_content(content: &str) -> Vec<String> {
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

pub fn graph_path_is_under(path: &str, root: &str) -> bool {
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
    candidates: &[GraphProjectionCandidate],
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
