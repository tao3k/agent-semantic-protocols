// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Provider-neutral project-resolution receipts and immutable package-graph facts.

use std::collections::HashSet;
use std::path::Component;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

use crate::LanguageIdV1;
use crate::ProviderIdV1;

/// Schema identifier for provider project-resolution receipts.
pub const PROJECT_RESOLUTION_SCHEMA_ID: &str = "agent.semantic-protocols.project-resolution";
/// Schema identifier for immutable language package graphs.
pub const LANGUAGE_PACKAGE_GRAPH_SCHEMA_ID: &str =
    "agent.semantic-protocols.language-package-graph";

/// Provider-owned package-manager facts. Repository and worktree identity are
/// deliberately absent; ASP binds this receipt to an admitted candidate base.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Raw DTO boundary for a provider project-resolution receipt.
///
/// Typed catalog boundary: `state` and completeness labels are validated against
/// the closed project-resolution catalog before ASP admits the receipt.
pub struct ProjectResolutionReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub state: String,
    pub completeness: String,
    pub language_id: String,
    pub provider_id: String,
    pub parser_id: String,
    pub candidate_generation_digest: String,
    pub project_entry: String,
    pub package_graph: LanguagePackageGraph,
    pub source_scopes: Vec<ResolvedSourceScope>,
    pub conflicts: Vec<ProjectResolutionConflict>,
    pub metrics: ProjectResolutionMetrics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Raw DTO boundary for an immutable language package graph.
pub struct LanguagePackageGraph {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub provider_id: String,
    pub project_entry: String,
    pub parser_id: String,
    pub manifests: Vec<ProjectFile>,
    pub lockfiles: Vec<ProjectFile>,
    pub packages: Vec<LanguagePackage>,
    pub internal_dependency_edges: Vec<InternalDependencyEdge>,
    pub external_dependencies: Vec<ExternalDependency>,
    pub unresolved: Vec<UnresolvedProjectReference>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Describes one project file classified by a language provider.
///
/// Typed catalog boundary: file kinds remain provider-owned vocabulary and are
/// admitted only as non-empty package-graph facts.
pub struct ProjectFile {
    pub path: String,
    pub kind: String,
    pub digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Describes one package discovered in a language project.
pub struct LanguagePackage {
    pub package_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub manifest_path: String,
    pub root: String,
    pub workspace_member: bool,
    pub targets: Vec<LanguageTarget>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Describes one build or execution target owned by a language package.
///
/// Typed catalog boundary: target kinds remain provider-owned vocabulary and
/// are admitted only with a valid target identity.
pub struct LanguageTarget {
    pub target_id: String,
    pub kind: String,
    pub name: String,
    pub explicit: bool,
    pub source_roots: Vec<String>,
    pub entrypoints: Vec<String>,
    pub generated_roots: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Describes one dependency edge between packages in the same project graph.
///
/// Typed catalog boundary: dependency kinds remain provider-owned vocabulary.
pub struct InternalDependencyEdge {
    pub from_package_id: String,
    pub to_package_id: String,
    pub kind: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Describes one dependency resolved outside the current project graph.
///
/// Typed catalog boundary: external dependency kinds remain provider-owned vocabulary.
pub struct ExternalDependency {
    pub dependency_id: String,
    pub name: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Describes a project reference that the provider could not resolve.
///
/// Typed catalog boundary: unresolved states and reasons preserve provider-owned vocabulary.
pub struct UnresolvedProjectReference {
    pub state: String,
    pub path: String,
    pub reason_kind: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Raw DTO boundary for an admitted source scope in a package graph.
///
/// Typed catalog boundary: optional resolution state preserves provider-owned vocabulary.
pub struct ResolvedSourceScope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,
    pub scope_id: String,
    pub package_id: String,
    pub target_id: String,
    pub roots: Vec<String>,
    pub explicit_paths: Vec<String>,
    pub extensions: Vec<String>,
    pub include_authority: String,
    #[serde(default)]
    pub classifications: Vec<String>,
    pub exclusions: Vec<ResolvedSourceExclusion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_facts: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<ProjectResolutionConflict>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_digest: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Describes one source exclusion attached to an admitted scope.
pub struct ResolvedSourceExclusion {
    pub prefix: String,
    pub authority: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Describes a deterministic conflict found during project resolution.
///
/// Typed catalog boundary: conflict reasons preserve provider-owned vocabulary.
pub struct ProjectResolutionConflict {
    pub path: String,
    pub include_authority: String,
    pub exclude_authority: String,
    pub reason_kind: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Records measured work performed by project resolution.
pub struct ProjectResolutionMetrics {
    pub parsed_manifest_count: u64,
    pub parsed_lockfile_count: u64,
    pub affected_package_count: u64,
    pub full_workspace_reads: u64,
    pub full_manifest_reparses: u64,
    pub db_opens: u64,
    /// JSON v1 metrics are bounded to the unsigned 64-bit integer domain.
    /// `serde_json` deliberately rejects `u128`, so a wider in-process timing
    /// value must be saturated before it can cross a provider/Runtime IPC edge.
    pub elapsed_micros: u64,
}

/// ASP-owned binding between an admitted candidate base and an unchanged
/// provider ProjectResolution receipt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdmittedProjectResolution {
    pub candidate_base: String,
    pub resolution_digest: String,
    pub resolution: ProjectResolutionReceipt,
}

impl ProjectResolutionReceipt {
    /// Validates the raw provider DTO against the expected ASP identity.
    ///
    /// Primitive field boundary: expected identifiers arrive from the provider
    /// registry and are compared without rewriting their wire representation.
    pub fn validate(
        &self,
        expected_language_id: &LanguageIdV1,
        expected_provider_id: &ProviderIdV1,
        expected_candidate_generation: &str,
    ) -> Result<(), String> {
        if self.schema_id != PROJECT_RESOLUTION_SCHEMA_ID
            || self.schema_version != "1"
            || self.state != "resolved"
            || !matches!(self.completeness.as_str(), "exact" | "complete")
        {
            return Err("provider ProjectResolution identity/state is not complete v1".to_owned());
        }
        if self.language_id != expected_language_id.as_str()
            || self.provider_id != expected_provider_id.as_str()
            || self.candidate_generation_digest != expected_candidate_generation
        {
            return Err(format!(
                "provider ProjectResolution authority identity drift: languageId expected={expected_language_id} actual={}; providerId expected={expected_provider_id} actual={}; candidateGenerationDigest expected={expected_candidate_generation} actual={}",
                self.language_id, self.provider_id, self.candidate_generation_digest
            ));
        }
        if self.parser_id.trim().is_empty() || !is_relative_logical_path(&self.project_entry) {
            return Err("provider ProjectResolution parser/project entry is invalid".to_owned());
        }
        let graph = &self.package_graph;
        if graph.schema_id != LANGUAGE_PACKAGE_GRAPH_SCHEMA_ID
            || graph.schema_version != "1"
            || graph.language_id != self.language_id
            || graph.provider_id != self.provider_id
            || graph.parser_id != self.parser_id
            || graph.project_entry != self.project_entry
        {
            return Err("provider ProjectResolution package graph identity drift".to_owned());
        }
        validate_package_graph(graph)?;
        validate_source_scopes(&self.source_scopes)?;
        if self.metrics.full_workspace_reads != 0
            || self.metrics.full_manifest_reparses != 0
            || self.metrics.db_opens != 0
        {
            return Err("provider ProjectResolution violated bounded parser metrics".to_owned());
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<String, String> {
        typed_digest(self)
    }
}

impl AdmittedProjectResolution {
    pub fn new(
        candidate_base: impl Into<String>,
        resolution: ProjectResolutionReceipt,
    ) -> Result<Self, String> {
        let candidate_base = candidate_base.into();
        if !is_relative_logical_path(&candidate_base) {
            return Err(
                "ASP ProjectResolution candidate base must be a relative logical path".to_owned(),
            );
        }
        let resolution_digest = resolution.digest()?;
        Ok(Self {
            candidate_base,
            resolution_digest,
            resolution,
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if !is_relative_logical_path(&self.candidate_base) {
            return Err(
                "ASP ProjectResolution candidate base must be a relative logical path".to_owned(),
            );
        }
        if self.resolution_digest != self.resolution.digest()? {
            return Err("ASP ProjectResolution receipt digest drift".to_owned());
        }
        self.resolution.validate(
            &self.resolution.language_id.as_str().into(),
            &self.resolution.provider_id.as_str().into(),
            &self.resolution.candidate_generation_digest,
        )
    }
}

/// Computes the generation digest for the complete admitted workspace source scope.
pub fn workspace_source_scope_generation_digest(
    resolutions: &[AdmittedProjectResolution],
) -> Result<String, String> {
    for resolution in resolutions {
        resolution.validate()?;
    }
    typed_digest(resolutions)
}

fn validate_package_graph(graph: &LanguagePackageGraph) -> Result<(), String> {
    validate_project_files(graph)?;
    let package_ids = collect_valid_package_ids(&graph.packages)?;
    validate_internal_dependency_edges(&graph.internal_dependency_edges, &package_ids)
}

fn validate_project_files(graph: &LanguagePackageGraph) -> Result<(), String> {
    graph
        .manifests
        .iter()
        .chain(&graph.lockfiles)
        .try_for_each(|file| {
            if !is_relative_logical_path(&file.path)
                || file.kind.trim().is_empty()
                || file.digest.trim().is_empty()
            {
                return Err(
                    "provider package graph contains an invalid manifest/lockfile".to_owned(),
                );
            }
            Ok(())
        })
}

fn collect_valid_package_ids(packages: &[LanguagePackage]) -> Result<HashSet<&str>, String> {
    let mut package_ids = HashSet::with_capacity(packages.len());
    for package in packages {
        if package.package_id.trim().is_empty()
            || package.name.trim().is_empty()
            || !is_relative_logical_path(&package.root)
            || !is_relative_logical_path(&package.manifest_path)
            || !package_ids.insert(package.package_id.as_str())
        {
            return Err(
                "provider package graph contains an invalid or duplicate package".to_owned(),
            );
        }
        package
            .targets
            .iter()
            .try_for_each(validate_language_target)?;
    }
    Ok(package_ids)
}

fn validate_language_target(target: &LanguageTarget) -> Result<(), String> {
    if target.target_id.trim().is_empty() || target.name.trim().is_empty() {
        return Err("provider package graph contains an invalid target".to_owned());
    }
    target
        .source_roots
        .iter()
        .chain(&target.entrypoints)
        .chain(&target.generated_roots)
        .try_for_each(|path| {
            if !is_relative_logical_path(path) {
                return Err("provider package graph target contains an invalid path".to_owned());
            }
            Ok(())
        })
}

fn validate_internal_dependency_edges(
    edges: &[InternalDependencyEdge],
    package_ids: &HashSet<&str>,
) -> Result<(), String> {
    edges.iter().try_for_each(|edge| {
        if !package_ids.contains(edge.from_package_id.as_str())
            || !package_ids.contains(edge.to_package_id.as_str())
        {
            return Err(
                "provider package graph dependency edge references an unknown package".to_owned(),
            );
        }
        Ok(())
    })
}

fn validate_source_scopes(scopes: &[ResolvedSourceScope]) -> Result<(), String> {
    let mut scope_ids = HashSet::with_capacity(scopes.len());
    for scope in scopes {
        if scope.scope_id.trim().is_empty()
            || scope.package_id.trim().is_empty()
            || scope.target_id.trim().is_empty()
            || scope.roots.is_empty()
            || scope.extensions.is_empty()
            || !scope_ids.insert(scope.scope_id.as_str())
        {
            return Err(
                "provider ProjectResolution contains an invalid or duplicate source scope"
                    .to_owned(),
            );
        }
        for path in scope
            .roots
            .iter()
            .chain(&scope.explicit_paths)
            .chain(scope.exclusions.iter().map(|exclusion| &exclusion.prefix))
        {
            if !is_relative_logical_path(path) {
                return Err(
                    "provider ProjectResolution source scope contains an invalid path".to_owned(),
                );
            }
        }
    }
    Ok(())
}

fn is_relative_logical_path(path: &str) -> bool {
    !path.trim().is_empty()
        && !Path::new(path).is_absolute()
        && !Path::new(path).components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}

fn typed_digest<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| format!("encode ProjectResolution digest input: {error}"))?;
    Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
}

/// Digest of the unique root ProjectResolution response schema distributed to
/// every language provider.
#[must_use]
pub fn project_resolution_schema_digest() -> String {
    format!(
        "blake3-256:{}",
        blake3::hash(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/provider-project-resolution-response.schema.json"
        )))
        .to_hex()
    )
}

#[cfg(test)]
#[path = "../tests/unit/project_resolution.rs"]
mod tests;
