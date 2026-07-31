//! Provider project-resolution execution and project-scope projection.

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
const PROJECT_SCOPE_PROVIDER_TIMEOUT_MS: u64 = 750;
const PROJECT_SCOPE_MAX_STDOUT_BYTES: usize = 1024 * 1024;
const PROJECT_SCOPE_MAX_STDERR_BYTES: usize = 128 * 1024;

/// Project-scope resolution result for a provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderProjectScope {
    Supported(ProviderProjectScopePacket),
    Unsupported,
}

/// Resolved project scope returned by a provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderProjectScopePacket {
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub files: Vec<ProviderProjectScopeFile>,
}

/// A file entry inside a provider project scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderProjectScopeFile {
    pub path: String,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
}

/// File-backed project-scope results after path resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderProjectScopeFiles {
    Supported(Vec<ProviderProjectScopePathFile>),
    Unsupported,
}

/// A resolved file path paired with provider identity metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderProjectScopePathFile {
    pub path: PathBuf,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
}

/// Resolve the provider project scope for a project root.
pub fn provider_project_scope(
    provider: &ResolvedProvider,
    project_root: &Path,
    _package_root: &str,
) -> Result<ProviderProjectScope, String> {
    let (backend, request) = provider_project_scope_invocation(provider, project_root)?;
    let output = backend
        .execute_with_limits(&request, project_scope_provider_limits())
        .map_err(|error| {
            format!(
                "provider project-resolution invocation failed: languageId={} providerId={} error={error}",
                provider.language_id, provider.provider_id
            )
        })?;
    provider_project_scope_from_output(output, provider)
}

pub async fn provider_project_scope_async(
    provider: &ResolvedProvider,
    project_root: &Path,
    _package_root: &str,
) -> Result<ProviderProjectScope, String> {
    let project_root_owned = project_root.to_path_buf();
    let repository_candidates = tokio::task::spawn_blocking(move || {
        agent_semantic_runtime::git::discover_repository_candidate_snapshot(&project_root_owned)
    })
    .await
    .map_err(|error| format!("provider Git candidate task failed: {error}"))?
    .map_err(|error| format!("discover provider repository candidates: {error}"))?
    .ok_or_else(|| {
        format!(
            "provider project resolution requires a Git candidate snapshot: workspace={}",
            project_root.display()
        )
    })?;
    provider_project_scope_with_candidates_async(provider, project_root, repository_candidates)
        .await
}

pub async fn provider_project_scope_with_candidates_async(
    provider: &ResolvedProvider,
    project_root: &Path,
    repository_candidates: agent_semantic_runtime::git::RepositoryCandidateSnapshot,
) -> Result<ProviderProjectScope, String> {
    let (backend, request) = provider_project_scope_invocation_with_candidates(
        provider,
        project_root,
        repository_candidates,
    )?;
    let output = backend
        .execute_with_limits_async(&request, project_scope_provider_limits())
        .await
        .map_err(|error| {
            format!(
                "provider project-resolution invocation failed: languageId={} providerId={} error={error}",
                provider.language_id, provider.provider_id
            )
        })?;
    provider_project_scope_from_output(output, provider)
}

fn provider_project_scope_invocation(
    provider: &ResolvedProvider,
    project_root: &Path,
) -> Result<(LocalNativeCliBackend, ClientRequest), String> {
    let repository_candidates =
        agent_semantic_runtime::git::discover_repository_candidate_snapshot(project_root)
            .map_err(|error| format!("discover provider repository candidates: {error}"))?
            .ok_or_else(|| {
                format!(
                    "provider project resolution requires a Git candidate snapshot: workspace={}",
                    project_root.display()
                )
            })?;
    provider_project_scope_invocation_with_candidates(
        provider,
        project_root,
        repository_candidates,
    )
}

fn provider_project_scope_invocation_with_candidates(
    provider: &ResolvedProvider,
    project_root: &Path,
    repository_candidates: agent_semantic_runtime::git::RepositoryCandidateSnapshot,
) -> Result<(LocalNativeCliBackend, ClientRequest), String> {
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
    Ok((LocalNativeCliBackend::new(snapshot), request))
}

fn provider_project_scope_from_output(
    output: crate::LocalNativeOutput,
    provider: &ResolvedProvider,
) -> Result<ProviderProjectScope, String> {
    if output.status_code != 0 {
        return Err(format!(
            "provider project-resolution failed: languageId={} providerId={} status={} stderr={}",
            provider.language_id,
            provider.provider_id,
            output.status_code,
            String::from_utf8_lossy(output.stderr.as_ref())
        ));
    }
    provider_project_scope_from_stdout(output.stdout.as_ref(), provider)
}

fn project_scope_provider_limits() -> ProviderProcessLimits {
    ProviderProcessLimits::new(
        Some(Duration::from_millis(PROJECT_SCOPE_PROVIDER_TIMEOUT_MS)),
        Some(PROJECT_SCOPE_MAX_STDOUT_BYTES),
        Some(PROJECT_SCOPE_MAX_STDERR_BYTES),
        Some(1024 * 1024 * 1024),
    )
}

/// Resolve file-backed project scope entries for a project root.
pub fn provider_project_scope_files(
    project_root: &Path,
    provider: &ResolvedProvider,
    package_root: &str,
    package_root_path: &Path,
) -> Result<ProviderProjectScopeFiles, String> {
    let ProviderProjectScope::Supported(packet) =
        provider_project_scope(provider, project_root, package_root)?
    else {
        return Ok(ProviderProjectScopeFiles::Unsupported);
    };
    Ok(ProviderProjectScopeFiles::Supported(
        provider_project_scope_files_from_packet(project_root, package_root_path, packet),
    ))
}

pub async fn provider_project_scope_files_async(
    project_root: &Path,
    provider: &ResolvedProvider,
    package_root: &str,
    package_root_path: &Path,
) -> Result<ProviderProjectScopeFiles, String> {
    let ProviderProjectScope::Supported(packet) =
        provider_project_scope_async(provider, project_root, package_root).await?
    else {
        return Ok(ProviderProjectScopeFiles::Unsupported);
    };
    Ok(ProviderProjectScopeFiles::Supported(
        provider_project_scope_files_from_packet_async(project_root, package_root_path, packet)
            .await?,
    ))
}

pub async fn provider_project_scope_files_with_candidates_async(
    project_root: &Path,
    provider: &ResolvedProvider,
    package_root_path: &Path,
    repository_candidates: agent_semantic_runtime::git::RepositoryCandidateSnapshot,
) -> Result<ProviderProjectScopeFiles, String> {
    let ProviderProjectScope::Supported(packet) = provider_project_scope_with_candidates_async(
        provider,
        project_root,
        repository_candidates,
    )
    .await?
    else {
        return Ok(ProviderProjectScopeFiles::Unsupported);
    };
    Ok(ProviderProjectScopeFiles::Supported(
        provider_project_scope_files_from_packet_async(project_root, package_root_path, packet)
            .await?,
    ))
}

#[cfg(test)]
#[path = "../tests/unit/provider_project_scope.rs"]
mod tests;

/// Convert a parsed project-resolution packet into resolved file paths.
#[must_use]
pub fn provider_project_scope_files_from_packet(
    project_root: &Path,
    package_root_path: &Path,
    packet: ProviderProjectScopePacket,
) -> Vec<ProviderProjectScopePathFile> {
    packet
        .files
        .into_iter()
        .filter_map(|file| {
            let path = scoped_child_path(package_root_path, &file.path)
                .filter(|path| path.is_file())
                .or_else(|| {
                    scoped_child_path(project_root, &file.path).filter(|path| path.is_file())
                })?;
            path.is_file().then_some(ProviderProjectScopePathFile {
                path,
                language_id: file.language_id,
                provider_id: file.provider_id,
            })
        })
        .collect()
}

async fn provider_project_scope_files_from_packet_async(
    project_root: &Path,
    package_root_path: &Path,
    packet: ProviderProjectScopePacket,
) -> Result<Vec<ProviderProjectScopePathFile>, String> {
    let concurrency = tokio::runtime::Handle::current().metrics().num_workers().max(1);
    let mut files = packet.files.into_iter();
    let mut tasks = tokio::task::JoinSet::new();
    let mut admitted = Vec::new();
    loop {
        while tasks.len() < concurrency {
            let Some(file) = files.next() else {
                break;
            };
            let package_candidate = scoped_child_path(package_root_path, &file.path);
            let workspace_candidate = scoped_child_path(project_root, &file.path);
            tasks.spawn(async move {
                for path in [package_candidate, workspace_candidate]
                    .into_iter()
                    .flatten()
                {
                    if tokio::fs::metadata(&path)
                        .await
                        .is_ok_and(|metadata| metadata.is_file())
                    {
                        return Some(ProviderProjectScopePathFile {
                            path,
                            language_id: file.language_id,
                            provider_id: file.provider_id,
                        });
                    }
                }
                None
            });
        }
        let Some(result) = tasks.join_next().await else {
            break;
        };
        if let Some(file) =
            result.map_err(|error| format!("provider scope metadata task failed: {error}"))?
        {
            admitted.push(file);
        }
    }
    admitted.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(admitted)
}

/// Parse a provider project-resolution response from stdout.
pub fn provider_project_scope_from_stdout(
    stdout: &[u8],
    provider: &ResolvedProvider,
) -> Result<ProviderProjectScope, String> {
    project_resolution_scope_from_stdout(stdout, &provider.language_id, &provider.provider_id)
}

fn project_resolution_scope_from_stdout(
    stdout: &[u8],
    expected_language_id: &LanguageId,
    expected_provider_id: &ProviderId,
) -> Result<ProviderProjectScope, String> {
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
        .candidates
        .iter()
        .map(|candidate| candidate.path.as_str())
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
        let policy_paths = resolution
            .repository_candidates
            .policy_exclusions
            .iter()
            .filter(|exclusion| {
                scope.roots.iter().any(|root| {
                    candidates_by_prefix
                        .get(root)
                        .is_some_and(|paths| paths.contains(&exclusion.path))
                }) && scope
                    .extensions
                    .iter()
                    .any(|extension| exclusion.path.ends_with(extension))
            })
            .collect::<Vec<_>>();
        if scope.include_authority == "manifest-explicit" {
            if let Some(exclusion) = policy_paths.first() {
                return Err(format!(
                    "project-scope-conflict: path={} includeAuthority=manifest-explicit excludeAuthority={} reasonKind=explicit-source-excluded",
                    exclusion.path, exclusion.authority
                ));
            }
        }
        for exclusion in policy_paths {
            scope_paths.remove(&exclusion.path);
        }
        resolved_paths.extend(scope_paths);
    }
    let files = resolved_paths
        .into_iter()
        .map(|path| ProviderProjectScopeFile {
            path,
            language_id: packet.language_id.clone(),
            provider_id: packet.provider_id.clone(),
        })
        .collect();
    Ok(ProviderProjectScope::Supported(
        ProviderProjectScopePacket {
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
    repository_candidates: RawRepositoryCandidateSnapshot,
    resolved_source_scopes: Vec<RawResolvedSourceScope>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRepositoryCandidateSnapshot {
    candidates: Vec<RawRepositoryCandidate>,
    policy_exclusions: Vec<RawRepositoryCandidatePolicyExclusion>,
}

#[derive(Debug, Deserialize)]
struct RawRepositoryCandidate {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRepositoryCandidatePolicyExclusion {
    path: String,
    authority: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawResolvedSourceScope {
    roots: Vec<String>,
    extensions: Vec<String>,
    include_authority: String,
    exclusions: Vec<RawResolvedSourceExclusion>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawResolvedSourceExclusion {
    prefix: String,
}
