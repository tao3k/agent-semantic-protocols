// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::Value;
use serde_json::json;

use crate::GraphProjectionCandidate;
use crate::stable_graph_node_id;

#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct GraphTopologyLanguageId(String);

impl AsRef<str> for GraphTopologyLanguageId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for GraphTopologyLanguageId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for GraphTopologyLanguageId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for GraphTopologyLanguageId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<&String> for GraphTopologyLanguageId {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

impl GraphTopologyLanguageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for GraphTopologyLanguageId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for GraphTopologyLanguageId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GraphTopologyProjection {
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
}

pub struct GraphTopologyProjectionRequest<'a> {
    language_id: GraphTopologyLanguageId,
    workspace_root: &'a Path,
    candidates: &'a [GraphProjectionCandidate],
    project_resolutions: &'a [agent_semantic_content_identity::AdmittedProjectResolution],
    include_repository_discovery: bool,
}

pub struct GraphOwnerMissingTopologyRequest<'a> {
    pub language_id: &'a str,
    pub owner_path: &'a str,
    pub generation_digest: &'a str,
    pub root_digest: &'a str,
    pub project_resolutions: &'a [agent_semantic_content_identity::AdmittedProjectResolution],
}

impl<'a> GraphTopologyProjectionRequest<'a> {
    pub fn new(
        language_id: &'a GraphTopologyLanguageId,
        workspace_root: &'a Path,
        candidates: &'a [GraphProjectionCandidate],
    ) -> Self {
        Self {
            language_id: language_id.clone(),
            workspace_root,
            candidates,
            project_resolutions: &[],
            include_repository_discovery: true,
        }
    }

    pub fn with_project_resolutions(
        mut self,
        project_resolutions: &'a [agent_semantic_content_identity::AdmittedProjectResolution],
    ) -> Self {
        self.project_resolutions = project_resolutions;
        self
    }

    pub fn without_repository_discovery(mut self) -> Self {
        self.include_repository_discovery = false;
        self
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
            language_id: language_id.into(),
            workspace_root,
            candidates,
            project_resolutions: &[],
            include_repository_discovery: true,
        }
    }
}

pub fn graph_project_topology_projection(
    request: GraphTopologyProjectionRequest<'_>,
) -> GraphTopologyProjection {
    let mut projection = GraphTopologyProjection::default();
    let (workspace_id, provider_id) =
        append_workspace_and_provider_root(&mut projection, &request.language_id);
    let submodule_paths = admitted_submodule_paths(&request);
    append_submodule_topology(&mut projection, &workspace_id, &submodule_paths);
    append_language_project_topology(&mut projection, &provider_id, &submodule_paths, &request);
    projection
}

fn append_workspace_and_provider_root(
    projection: &mut GraphTopologyProjection,
    language_id: &GraphTopologyLanguageId,
) -> (String, String) {
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
            "languageId": language_id.as_str(),
        },
    }));
    projection.edges.push(json!({
        "source": workspace_id.clone(),
        "target": provider_id.clone(),
        "relation": "has_provider_root",
    }));
    (workspace_id, provider_id)
}

fn admitted_submodule_paths(request: &GraphTopologyProjectionRequest<'_>) -> Vec<String> {
    if request.include_repository_discovery {
        graph_project_submodule_paths(request.workspace_root)
    } else {
        Vec::new()
    }
}

fn append_submodule_topology(
    projection: &mut GraphTopologyProjection,
    workspace_id: &str,
    submodule_paths: &[String],
) {
    for submodule_path in submodule_paths {
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
}

fn append_language_project_topology(
    projection: &mut GraphTopologyProjection,
    provider_id: &str,
    submodule_paths: &[String],
    request: &GraphTopologyProjectionRequest<'_>,
) {
    for admitted in request
        .project_resolutions
        .iter()
        .filter(|admitted| admitted.resolution.language_id == request.language_id.as_str())
    {
        append_admitted_project_resolution(
            projection,
            provider_id,
            &request.language_id,
            admitted,
            request.candidates,
            submodule_paths,
        );
    }
}

pub fn graph_owner_missing_topology_projection(
    request: GraphOwnerMissingTopologyRequest<'_>,
) -> GraphTopologyProjection {
    const PROJECT_LIMIT: usize = 3;
    const SCOPE_LIMIT: usize = 4;

    let mut projection = GraphTopologyProjection::default();
    let workspace_id = stable_graph_node_id("workspace", ".");
    let provider_root_value = format!("{}:.", request.language_id);
    let provider_root_id = stable_graph_node_id("provider-root", &provider_root_value);
    let generation_id = stable_graph_node_id("generation", request.generation_digest);
    let owner_id = stable_graph_node_id("owner", request.owner_path);
    projection.nodes.extend([
        json!({
            "id": workspace_id.clone(),
            "kind": "workspace",
            "role": "root",
            "value": ".",
            "action": "topology",
            "confidence": "exact",
        }),
        json!({
            "id": provider_root_id.clone(),
            "kind": "provider-root",
            "role": "language-root",
            "value": provider_root_value,
            "action": "topology",
            "confidence": "exact",
            "fields": { "languageId": request.language_id },
        }),
        json!({
            "id": generation_id.clone(),
            "kind": "generation",
            "role": "workspace-generation",
            "value": request.generation_digest,
            "action": "topology",
            "confidence": "exact",
            "fields": { "rootDigest": request.root_digest },
        }),
        json!({
            "id": owner_id.clone(),
            "kind": "owner",
            "role": "missing-owner",
            "value": request.owner_path,
            "path": request.owner_path,
            "action": "owner",
            "confidence": "exact",
            "fields": {
                "languageId": request.language_id,
                "state": "owner-missing",
                "reasonKind": "owner-not-in-workspace",
            },
        }),
    ]);
    projection.edges.extend([
        json!({
            "source": workspace_id,
            "target": provider_root_id.clone(),
            "relation": "has_provider_root",
        }),
        json!({
            "source": generation_id,
            "target": owner_id.clone(),
            "relation": "omits_owner",
        }),
    ]);

    let mut scope_count = 0;
    for (project_index, admitted) in request
        .project_resolutions
        .iter()
        .filter(|admitted| {
            admitted.resolution.language_id == request.language_id
                && admitted_project_resolution_contains_owner(admitted, request.owner_path)
        })
        .enumerate()
    {
        if project_index == PROJECT_LIMIT || scope_count == SCOPE_LIMIT {
            break;
        }
        let resolution = &admitted.resolution;
        let project_entry = logical_join(&admitted.candidate_base, &resolution.project_entry);
        let project_value = format!(
            "{}:{}:{project_entry}",
            request.language_id, resolution.provider_id
        );
        let project_id = stable_graph_node_id("language-project", &project_value);
        projection.nodes.push(json!({
            "id": project_id.clone(),
            "kind": "language-project",
            "role": "project-root",
            "value": project_entry,
            "path": project_entry,
            "action": "topology",
            "confidence": "exact",
            "fields": {
                "languageId": request.language_id,
                "providerId": resolution.provider_id,
                "parserId": resolution.parser_id,
                "resolutionDigest": admitted.resolution_digest,
            },
        }));
        projection.edges.push(json!({
            "source": provider_root_id,
            "target": project_id.clone(),
            "relation": "has_language_project",
        }));

        for scope in resolution.source_scopes.iter().filter(|scope| {
            source_scope_contains(&admitted.candidate_base, scope, request.owner_path)
        }) {
            if scope_count == SCOPE_LIMIT {
                break;
            }
            scope_count += 1;
            let scope_id = stable_graph_node_id(
                "source-scope",
                &format!("{project_value}:{}", scope.scope_id),
            );
            projection.nodes.push(json!({
                "id": scope_id.clone(),
                "kind": "source-scope",
                "role": "provider-resolved",
                "value": scope.scope_id,
                "action": "owner",
                "confidence": "exact",
                "fields": {
                    "packageId": scope.package_id,
                    "targetId": scope.target_id,
                    "includeAuthority": scope.include_authority,
                    "scopeDigest": scope.scope_digest,
                },
            }));
            projection.edges.extend([
                json!({
                    "source": project_id,
                    "target": scope_id.clone(),
                    "relation": "resolves_source_scope",
                }),
                json!({
                    "source": scope_id,
                    "target": owner_id.clone(),
                    "relation": "expects_owner",
                }),
            ]);
        }
    }
    projection
}

fn admitted_project_resolution_contains_owner(
    admitted: &agent_semantic_content_identity::AdmittedProjectResolution,
    owner_path: &str,
) -> bool {
    admitted
        .resolution
        .source_scopes
        .iter()
        .any(|scope| source_scope_contains(&admitted.candidate_base, scope, owner_path))
}

fn append_admitted_project_resolution(
    projection: &mut GraphTopologyProjection,
    provider_root_id: &str,
    language_id: &GraphTopologyLanguageId,
    admitted: &agent_semantic_content_identity::AdmittedProjectResolution,
    candidates: &[GraphProjectionCandidate],
    submodule_paths: &[String],
) {
    let resolution = &admitted.resolution;
    let project_entry = logical_join(&admitted.candidate_base, &resolution.project_entry);
    let project_value = format!("{language_id}:{}:{project_entry}", resolution.provider_id);
    let project_id = stable_graph_node_id("language-project", &project_value);
    projection.nodes.push(json!({
        "id": project_id.clone(),
        "kind": "language-project",
        "role": "project-root",
        "value": project_entry,
        "action": "topology",
        "path": project_entry,
        "confidence": "exact",
        "fields": {
            "languageId": language_id,
            "providerId": resolution.provider_id,
            "parserId": resolution.parser_id,
            "candidateBase": admitted.candidate_base,
            "resolutionDigest": admitted.resolution_digest,
            "completeness": resolution.completeness,
        },
    }));
    projection.edges.push(json!({
        "source": provider_root_id,
        "target": project_id,
        "relation": "has_language_project",
    }));

    for manifest in &resolution.package_graph.manifests {
        append_project_file_node(
            projection,
            language_id,
            &project_id,
            &admitted.candidate_base,
            manifest,
            "project-marker",
            "declared_by",
        );
    }

    for lockfile in &resolution.package_graph.lockfiles {
        append_project_file_node(
            projection,
            language_id,
            &project_id,
            &admitted.candidate_base,
            lockfile,
            "dependency-marker",
            "uses_dependency_marker",
        );
    }

    for submodule_path in submodule_paths {
        if graph_path_is_under(&project_entry, submodule_path) {
            projection.edges.push(json!({
                "source": stable_graph_node_id("submodule", submodule_path),
                "target": project_id,
                "relation": "contains_project",
            }));
        }
    }

    for package in &resolution.package_graph.packages {
        let package_value = format!("{project_value}:{}", package.package_id);
        let package_id = stable_graph_node_id("package", &package_value);
        let package_root = logical_join(&admitted.candidate_base, &package.root);
        projection.nodes.push(json!({
            "id": package_id.clone(),
            "kind": "package",
            "role": "language-package",
            "value": package.name,
            "action": "topology",
            "path": package_root,
            "confidence": "exact",
            "fields": {
                "languageId": language_id,
                "providerId": resolution.provider_id,
                "packageId": package.package_id,
                "manifestPath": logical_join(&admitted.candidate_base, &package.manifest_path),
                "workspaceMember": package.workspace_member,
            },
        }));
        projection.edges.push(json!({
            "source": project_id,
            "target": package_id,
            "relation": "contains_package",
        }));

        for target in &package.targets {
            let target_value = format!("{package_value}:{}", target.target_id);
            let target_id = stable_graph_node_id("target", &target_value);
            projection.nodes.push(json!({
                "id": target_id.clone(),
                "kind": "target",
                "role": target.kind,
                "value": target.name,
                "action": "topology",
                "confidence": "exact",
                "fields": {
                    "languageId": language_id,
                    "packageId": package.package_id,
                    "targetId": target.target_id,
                    "explicit": target.explicit,
                    "sourceRoots": rebase_paths(&admitted.candidate_base, &target.source_roots),
                    "entrypoints": rebase_paths(&admitted.candidate_base, &target.entrypoints),
                    "generatedRoots": rebase_paths(&admitted.candidate_base, &target.generated_roots),
                },
            }));
            projection.edges.push(json!({
                "source": package_id,
                "target": target_id,
                "relation": "declares_target",
            }));
        }
    }

    append_internal_dependency_edges(projection, &project_value, resolution);
    append_source_scope_nodes(
        projection,
        &project_value,
        &admitted.candidate_base,
        resolution,
        candidates,
    );
}

fn append_project_file_node(
    projection: &mut GraphTopologyProjection,
    language_id: &GraphTopologyLanguageId,
    project_id: &str,
    candidate_base: &str,
    file: &agent_semantic_content_identity::ProjectFile,
    kind: &str,
    relation: &str,
) {
    let path = logical_join(candidate_base, &file.path);
    let marker_id = stable_graph_node_id(kind, &format!("{language_id}:{path}"));
    projection.nodes.push(json!({
        "id": marker_id.clone(),
        "kind": kind,
        "role": file.kind,
        "value": path,
        "action": "topology",
        "path": path,
        "confidence": "exact",
        "fields": {
            "languageId": language_id,
            "digest": file.digest,
        },
    }));
    projection.edges.push(json!({
        "source": project_id,
        "target": marker_id,
        "relation": relation,
    }));
}

fn append_internal_dependency_edges(
    projection: &mut GraphTopologyProjection,
    project_value: &str,
    resolution: &agent_semantic_content_identity::ProjectResolutionReceipt,
) {
    for dependency in &resolution.package_graph.internal_dependency_edges {
        projection.edges.push(json!({
            "source": stable_graph_node_id(
                "package",
                &format!("{project_value}:{}", dependency.from_package_id),
            ),
            "target": stable_graph_node_id(
                "package",
                &format!("{project_value}:{}", dependency.to_package_id),
            ),
            "relation": "depends_on_package",
            "fields": { "dependencyKind": dependency.kind },
        }));
    }
}

fn append_source_scope_nodes(
    projection: &mut GraphTopologyProjection,
    project_value: &str,
    candidate_base: &str,
    resolution: &agent_semantic_content_identity::ProjectResolutionReceipt,
    candidates: &[GraphProjectionCandidate],
) {
    for scope in &resolution.source_scopes {
        let scope_id = stable_graph_node_id(
            "source-scope",
            &format!("{project_value}:{}", scope.scope_id),
        );
        projection.nodes.push(json!({
            "id": scope_id.clone(),
            "kind": "source-scope",
            "role": "provider-resolved",
            "value": scope.scope_id,
            "action": "owner",
            "confidence": "exact",
            "fields": {
                "packageId": scope.package_id,
                "targetId": scope.target_id,
                "roots": rebase_paths(candidate_base, &scope.roots),
                "explicitPaths": rebase_paths(candidate_base, &scope.explicit_paths),
                "extensions": scope.extensions,
                "includeAuthority": scope.include_authority,
                "scopeDigest": scope.scope_digest,
            },
        }));
        projection.edges.push(json!({
            "source": stable_graph_node_id(
                "target",
                &format!("{project_value}:{}:{}", scope.package_id, scope.target_id),
            ),
            "target": scope_id,
            "relation": "resolves_source_scope",
        }));
        for candidate in candidates {
            if source_scope_contains(candidate_base, scope, &candidate.path) {
                projection.edges.push(json!({
                    "source": scope_id,
                    "target": stable_graph_node_id("owner", &candidate.path),
                    "relation": "admits_owner",
                }));
            }
        }
    }
}

fn source_scope_contains(
    candidate_base: &str,
    scope: &agent_semantic_content_identity::ResolvedSourceScope,
    candidate_path: &str,
) -> bool {
    let explicit_match = scope
        .explicit_paths
        .iter()
        .map(|path| logical_join(candidate_base, path))
        .any(|path| candidate_path == path);
    let root_match = scope
        .roots
        .iter()
        .map(|root| logical_join(candidate_base, root))
        .any(|root| graph_path_is_under(candidate_path, &root));
    let excluded = scope
        .exclusions
        .iter()
        .map(|exclusion| logical_join(candidate_base, &exclusion.prefix))
        .any(|prefix| graph_path_is_under(candidate_path, &prefix));
    let extension_matches = scope.extensions.is_empty()
        || scope.extensions.iter().any(|extension| {
            let extension = extension.trim_start_matches('.');
            candidate_path.ends_with(&format!(".{extension}"))
        });
    (explicit_match || root_match) && !excluded && extension_matches
}

fn rebase_paths(candidate_base: &str, paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .map(|path| logical_join(candidate_base, path))
        .collect()
}

fn logical_join(base: &str, path: &str) -> String {
    let base = base.trim_matches('/');
    let path = path.trim_start_matches("./").trim_matches('/');
    if base.is_empty() || base == "." {
        path.to_owned()
    } else if path.is_empty() || path == "." {
        base.to_owned()
    } else {
        format!("{base}/{path}")
    }
}

pub fn graph_submodule_owner_edges(workspace_root: &Path, owners: &[String]) -> Vec<Value> {
    let submodule_paths = graph_project_submodule_paths(workspace_root);
    if submodule_paths.is_empty() {
        return Vec::new();
    }
    let submodule_index = submodule_paths
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut edges = Vec::new();
    for owner in owners {
        let Some(submodule_path) = deepest_indexed_owner_prefix(owner, &submodule_index) else {
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

fn deepest_indexed_owner_prefix<'a>(
    owner: &str,
    submodule_index: &BTreeSet<&'a str>,
) -> Option<&'a str> {
    let mut prefix_end = owner.len();
    loop {
        let prefix = &owner[..prefix_end];
        if let Some(indexed) = submodule_index.get(prefix) {
            return Some(*indexed);
        }
        let separator = prefix.rfind('/')?;
        prefix_end = separator;
    }
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
