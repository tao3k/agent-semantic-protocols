//! Provider workspace-scope packet execution and parsing.

use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_semantic_client_core::{
    ClientMethod, ClientRequest, LanguageId, ProviderId, ProviderRegistrySnapshot,
    ResolvedProvider, scoped_child_path,
};
use agent_semantic_provider_transport::ProviderProcessLimits;
use serde::Deserialize;

use crate::LocalNativeCliBackend;

const PROVIDER_PROJECT_RESOLUTION_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-project-resolution-response";
const PROJECT_RESOLUTION_SCHEMA_ID: &str = "agent.semantic-protocols.project-resolution";
const WORKSPACE_SCOPE_PROVIDER_TIMEOUT_MS: u64 = 750;
const WORKSPACE_SCOPE_MAX_STDOUT_BYTES: usize = 1024 * 1024;
const WORKSPACE_SCOPE_MAX_STDERR_BYTES: usize = 128 * 1024;

/// Workspace-scope resolution result for a provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderWorkspaceScope {
    Supported(ProviderWorkspaceScopePacket),
    Unsupported,
}

/// Resolved workspace-scope packet returned by a provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderWorkspaceScopePacket {
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub files: Vec<ProviderWorkspaceScopeFile>,
}

/// A file entry inside a provider workspace-scope packet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderWorkspaceScopeFile {
    pub path: String,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
}

/// File-backed workspace-scope results after path resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderWorkspaceScopeFiles {
    Supported(Vec<ProviderWorkspaceScopePathFile>),
    Unsupported,
}

/// A resolved file path paired with provider identity metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderWorkspaceScopePathFile {
    pub path: PathBuf,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
}

/// Resolve the provider workspace scope for a project root.
pub fn provider_workspace_scope(
    provider: &ResolvedProvider,
    project_root: &Path,
    _package_root: &str,
) -> Result<ProviderWorkspaceScope, String> {
    let repository_candidates =
        agent_semantic_runtime::git::discover_repository_candidate_snapshot(project_root)
            .map_err(|error| format!("discover provider repository candidates: {error}"))?
            .ok_or_else(|| {
                format!(
                    "provider project resolution requires a Git candidate snapshot: workspace={}",
                    project_root.display()
                )
            })?;
    let stdin = serde_json::to_vec(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.provider-project-resolution-request",
        "schemaVersion": "1",
        "languageId": provider.language_id.as_str(),
        "providerId": provider.provider_id.as_str(),
        "workspaceRoot": project_root,
        "repositoryCandidates": repository_candidates,
    }))
    .map_err(|error| format!("encode provider project-resolution request: {error}"))?;
    let request = ClientRequest::new(ClientMethod::ProjectResolution, project_root.to_path_buf())
        .with_language(provider.language_id.clone())
        .with_stdin(stdin);
    let snapshot = ProviderRegistrySnapshot {
        activation_path: PathBuf::new(),
        providers: vec![provider.clone()],
    };
    let output = match LocalNativeCliBackend::new(snapshot)
        .execute_with_limits(&request, workspace_scope_provider_limits())
    {
        Ok(output) => output,
        Err(error) => {
            return Err(format!(
                "provider project-resolution invocation failed: languageId={} providerId={} error={error}",
                provider.language_id, provider.provider_id
            ));
        }
    };
    if output.status_code != 0 {
        return Err(format!(
            "provider project-resolution failed: languageId={} providerId={} status={} stderr={}",
            provider.language_id,
            provider.provider_id,
            output.status_code,
            String::from_utf8_lossy(output.stderr.as_ref())
        ));
    }
    provider_workspace_scope_from_stdout(output.stdout.as_ref(), provider)
}

fn workspace_scope_provider_limits() -> ProviderProcessLimits {
    ProviderProcessLimits::new(
        Some(Duration::from_millis(WORKSPACE_SCOPE_PROVIDER_TIMEOUT_MS)),
        Some(WORKSPACE_SCOPE_MAX_STDOUT_BYTES),
        Some(WORKSPACE_SCOPE_MAX_STDERR_BYTES),
        Some(1024 * 1024 * 1024),
    )
}

/// Resolve file-backed workspace scope entries for a project root.
pub fn provider_workspace_scope_files(
    project_root: &Path,
    provider: &ResolvedProvider,
    package_root: &str,
    package_root_path: &Path,
) -> Result<ProviderWorkspaceScopeFiles, String> {
    let ProviderWorkspaceScope::Supported(packet) =
        provider_workspace_scope(provider, project_root, package_root)?
    else {
        return Ok(ProviderWorkspaceScopeFiles::Unsupported);
    };
    Ok(ProviderWorkspaceScopeFiles::Supported(
        provider_workspace_scope_files_from_packet(project_root, package_root_path, packet),
    ))
}

#[cfg(test)]
#[path = "../tests/unit/provider_workspace_scope.rs"]
mod tests;

/// Convert a parsed workspace-scope packet into resolved file paths.
#[must_use]
pub fn provider_workspace_scope_files_from_packet(
    project_root: &Path,
    package_root_path: &Path,
    packet: ProviderWorkspaceScopePacket,
) -> Vec<ProviderWorkspaceScopePathFile> {
    packet
        .files
        .into_iter()
        .filter_map(|file| {
            let path = scoped_child_path(package_root_path, &file.path)
                .filter(|path| path.is_file())
                .or_else(|| {
                    scoped_child_path(project_root, &file.path).filter(|path| path.is_file())
                })?;
            path.is_file().then_some(ProviderWorkspaceScopePathFile {
                path,
                language_id: file.language_id,
                provider_id: file.provider_id,
            })
        })
        .collect()
}

/// Parse a provider project-resolution response from stdout.
pub fn provider_workspace_scope_from_stdout(
    stdout: &[u8],
    provider: &ResolvedProvider,
) -> Result<ProviderWorkspaceScope, String> {
    project_resolution_scope_from_stdout(stdout, &provider.language_id, &provider.provider_id)
}

fn project_resolution_scope_from_stdout(
    stdout: &[u8],
    expected_language_id: &LanguageId,
    expected_provider_id: &ProviderId,
) -> Result<ProviderWorkspaceScope, String> {
    let stdout = String::from_utf8_lossy(stdout);
    let packet = serde_json::from_str::<RawProviderProjectResolutionResponse>(stdout.trim())
        .map_err(|error| format!("decode provider project-resolution response: {error}"))?;
    if packet.schema_id != PROVIDER_PROJECT_RESOLUTION_RESPONSE_SCHEMA_ID
        || packet.schema_version != "1"
    {
        return Err(format!(
            "unexpected provider project-resolution response schema: schemaId={} schemaVersion={}",
            packet.schema_id, packet.schema_version
        ));
    }
    if packet.language_id != *expected_language_id || packet.provider_id != *expected_provider_id {
        return Err(format!(
            "provider project-resolution identity mismatch: expectedLanguageId={} actualLanguageId={} expectedProviderId={} actualProviderId={}",
            expected_language_id, packet.language_id, expected_provider_id, packet.provider_id
        ));
    }
    if packet.state != "resolved" {
        return Err(format!(
            "provider project-resolution did not resolve: languageId={} providerId={} state={}",
            packet.language_id, packet.provider_id, packet.state
        ));
    }
    let resolution = packet.resolution.ok_or_else(|| {
        "resolved provider project-resolution response omitted resolution".to_string()
    })?;
    if resolution.schema_id != PROJECT_RESOLUTION_SCHEMA_ID
        || resolution.schema_version != "1"
        || resolution.state != "resolved"
        || resolution.completeness != "exact"
    {
        return Err(format!(
            "invalid project-resolution contract: schemaId={} schemaVersion={} state={} completeness={}",
            resolution.schema_id,
            resolution.schema_version,
            resolution.state,
            resolution.completeness
        ));
    }
    let candidate_paths = resolution
        .repository_candidates
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            "project-resolution repositoryCandidates omitted candidates".to_string()
        })?
        .iter()
        .filter_map(|candidate| {
            candidate
                .get("path")
                .and_then(serde_json::Value::as_str)
        })
        .collect::<Vec<_>>();
    let mut candidates_by_prefix: std::collections::BTreeMap<
        String,
        std::collections::BTreeSet<String>,
    > = std::collections::BTreeMap::new();
    let mut candidates_by_extension: std::collections::BTreeMap<
        String,
        std::collections::BTreeSet<String>,
    > = std::collections::BTreeMap::new();
    for candidate in &candidate_paths {
        candidates_by_prefix
            .entry(".".to_string())
            .or_default()
            .insert((*candidate).to_string());
        let components = candidate.split('/').collect::<Vec<_>>();
        for component_count in 1..=components.len() {
            candidates_by_prefix
                .entry(components[..component_count].join("/"))
                .or_default()
                .insert((*candidate).to_string());
        }
        let basename = candidate.rsplit('/').next().unwrap_or(candidate);
        for (index, character) in basename.char_indices() {
            if character == '.' {
                candidates_by_extension
                    .entry(basename[index..].to_string())
                    .or_default()
                    .insert((*candidate).to_string());
            }
        }
    }
    let mut resolved_paths = std::collections::BTreeSet::new();
    for scope in resolution.resolved_source_scopes {
        if scope.roots.is_empty() || scope.extensions.is_empty() {
            return Err(
                "project-resolution source scope must declare roots and extensions".to_string(),
            );
        }
        let mut scope_paths = std::collections::BTreeSet::new();
        for root in &scope.roots {
            if let Some(paths) = candidates_by_prefix.get(root) {
                scope_paths.extend(paths.iter().cloned());
            }
        }
        let mut extension_paths = std::collections::BTreeSet::new();
        for extension in &scope.extensions {
            if let Some(paths) = candidates_by_extension.get(extension) {
                extension_paths.extend(paths.iter().cloned());
            }
        }
        scope_paths.retain(|path| extension_paths.contains(path));
        for exclusion in &scope.exclusions {
            if let Some(paths) = candidates_by_prefix.get(&exclusion.prefix) {
                for path in paths {
                    scope_paths.remove(path);
                }
            }
        }
        resolved_paths.extend(scope_paths);
    }
    let files = resolved_paths
        .into_iter()
        .map(|path| ProviderWorkspaceScopeFile {
            path,
            language_id: packet.language_id.clone(),
            provider_id: packet.provider_id.clone(),
        })
        .collect();
    Ok(ProviderWorkspaceScope::Supported(
        ProviderWorkspaceScopePacket {
            language_id: packet.language_id,
            provider_id: packet.provider_id,
            files,
        },
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawProviderProjectResolutionResponse {
    schema_id: String,
    schema_version: String,
    language_id: LanguageId,
    provider_id: ProviderId,
    state: String,
    resolution: Option<RawProjectResolution>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawProjectResolution {
    schema_id: String,
    schema_version: String,
    state: String,
    completeness: String,
    repository_candidates: serde_json::Value,
    resolved_source_scopes: Vec<RawResolvedSourceScope>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawResolvedSourceScope {
    roots: Vec<String>,
    extensions: Vec<String>,
    exclusions: Vec<RawResolvedSourceExclusion>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawResolvedSourceExclusion {
    prefix: String,
}
