//! Provider ProjectResolution execution and source-file projection.

use agent_semantic_client_core::LanguageId;
use agent_semantic_client_core::ProviderId;
use agent_semantic_client_core::scoped_child_path;
use serde::Deserialize;
use std::path::Path;
use std::path::PathBuf;

const PROVIDER_PROJECT_RESOLUTION_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-project-resolution-response";

/// Project-scope resolution result for a provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderProjectResolution {
    Supported(ProviderProjectResolutionPacket),
    Unsupported,
}

/// Resolved project scope returned by a provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderProjectResolutionPacket {
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub resolution: agent_semantic_content_identity::ProjectResolutionReceipt,
    pub files: Vec<ProviderProjectResolutionFile>,
}

/// A file entry inside a provider project scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderProjectResolutionFile {
    pub path: String,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
}

/// File-backed project-resolution results after path resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderProjectResolutionFiles {
    Supported(Vec<ProviderProjectResolutionPathFile>),
    Unsupported,
}

/// A resolved file path paired with provider identity metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderProjectResolutionPathFile {
    pub path: PathBuf,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderProjectResolutionCandidates {
    pub generation_digest: String,
    generation: serde_json::Value,
    pub paths: Vec<String>,
    pub policy_exclusions: Vec<ProviderProjectResolutionPolicyExclusion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderProjectResolutionPolicyExclusion {
    pub path: String,
    pub authority: String,
    pub reason_kind: String,
}

pub fn provider_project_resolution_candidates(
    project_root: &Path,
    repository_candidates: &agent_semantic_runtime::git::RepositoryCandidateSnapshot,
) -> Result<ProviderProjectResolutionCandidates, String> {
    let project_root = std::fs::canonicalize(project_root)
        .map_err(|error| format!("canonicalize provider project root: {error}"))?;
    let repository_candidates = repository_candidates
        .clone()
        .scoped_to_project_root(&project_root)
        .map_err(|error| format!("scope provider repository candidates: {error}"))?;
    Ok(ProviderProjectResolutionCandidates {
        generation_digest: repository_candidates.candidate_generation.digest.clone(),
        generation: serde_json::to_value(&repository_candidates.candidate_generation)
            .map_err(|error| format!("encode candidate generation: {error}"))?,
        paths: repository_candidates
            .candidates
            .iter()
            .map(|candidate| candidate.path.to_string_lossy().replace('\\', "/"))
            .collect(),
        policy_exclusions: repository_candidates
            .policy_exclusions
            .iter()
            .map(|exclusion| ProviderProjectResolutionPolicyExclusion {
                path: exclusion.path.to_string_lossy().replace('\\', "/"),
                authority: exclusion.authority.clone(),
                reason_kind: exclusion.reason_kind.clone(),
            })
            .collect(),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderProjectResolutionCollectionScope {
    CompleteGeneration,
    ExplicitOwners { owner_paths: Vec<String> },
}

impl ProviderProjectResolutionCollectionScope {
    fn as_json(&self) -> serde_json::Value {
        match self {
            Self::CompleteGeneration => serde_json::json!({ "kind": "complete-generation" }),
            Self::ExplicitOwners { owner_paths } => serde_json::json!({
                "kind": "explicit-owners",
                "ownerPaths": owner_paths,
            }),
        }
    }
}

pub fn encode_provider_project_resolution_request(
    project_root: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
    collection_scope: &ProviderProjectResolutionCollectionScope,
    repository_candidates: &agent_semantic_runtime::git::RepositoryCandidateSnapshot,
) -> Result<(Vec<u8>, ProviderProjectResolutionCandidates), String> {
    let candidates = provider_project_resolution_candidates(project_root, repository_candidates)?;
    let stdin = serde_json::to_vec(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.provider-project-resolution-request",
        "schemaVersion": "1",
        "languageId": language_id.as_str(),
        "providerId": provider_id.as_str(),
        "candidateBase": ".",
    "candidateGeneration": candidates.generation,
    "collectionScope": collection_scope.as_json(),
        "candidatePaths": candidates.paths,
        "policyExclusions": candidates.policy_exclusions.iter().map(|exclusion| serde_json::json!({
            "path": exclusion.path,
            "authority": exclusion.authority,
            "reasonKind": exclusion.reason_kind,
        })).collect::<Vec<_>>(),
    }))
    .map_err(|error| format!("encode provider project-resolution request: {error}"))?;
    Ok((stdin, candidates))
}

/// Resolve the provider project scope for a project root.
/// Returns whether the resolved provider declared the project-resolution adapter.
pub fn provider_capabilities_permit_project_resolution(
    capabilities: &agent_semantic_client_core::ProviderSourceInventoryCapabilities,
) -> bool {
    capabilities.project_resolution.is_some()
}

/// Convert a parsed ProjectResolution packet into resolved file paths.
#[must_use]
pub fn provider_project_resolution_files_from_packet(
    project_root: &Path,
    package_root_path: &Path,
    packet: ProviderProjectResolutionPacket,
) -> Vec<ProviderProjectResolutionPathFile> {
    packet
        .files
        .into_iter()
        .filter_map(|file| {
            let path = scoped_child_path(package_root_path, &file.path)
                .filter(|path| path.is_file())
                .or_else(|| {
                    scoped_child_path(project_root, &file.path).filter(|path| path.is_file())
                })?;
            path.is_file().then_some(ProviderProjectResolutionPathFile {
                path,
                language_id: file.language_id,
                provider_id: file.provider_id,
            })
        })
        .collect()
}

pub async fn provider_project_resolution_files_from_packet_async(
    project_root: &Path,
    package_root_path: &Path,
    packet: ProviderProjectResolutionPacket,
) -> Result<Vec<ProviderProjectResolutionPathFile>, String> {
    let concurrency = tokio::runtime::Handle::current()
        .metrics()
        .num_workers()
        .max(1);
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
                        return Some(ProviderProjectResolutionPathFile {
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

/// Parse and validate a provider ProjectResolution response against ASP-owned candidates.
///
/// The provider response envelope carries the resolved project graph in `scope`.
/// `ProjectResolution` names the capability and schema, not an alternate
/// `resolution` envelope field.
pub fn project_resolution_from_stdout(
    stdout: &[u8],
    expected_language_id: &LanguageId,
    expected_provider_id: &ProviderId,
    candidates: &ProviderProjectResolutionCandidates,
) -> Result<ProviderProjectResolution, String> {
    let stdout = String::from_utf8_lossy(stdout);
    let trimmed = stdout.trim();
    let preview: String = trimmed.chars().take(256).collect();
    let packet = serde_json::from_str::<RawProviderProjectResolutionResponse>(trimmed).map_err(
    |error| {
        format!(
            "decode provider project-resolution response: {error}; bytes={}; prefix={preview:?}",
            stdout.len()
        )
    },
)?;
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
    if packet.state == "not-applicable" {
        if packet.scope.is_some() || packet.failure.is_some() {
            return Err(format!(
                "provider project-resolution not-applicable response must omit scope and failure: languageId={} providerId={}",
                packet.language_id, packet.provider_id
            ));
        }
        return Ok(ProviderProjectResolution::Unsupported);
    }
    if packet.state != "resolved" {
        let failure = packet.failure.ok_or_else(|| {
            format!(
                "provider project-resolution state={} omitted typed failure",
                packet.state
            )
        })?;
        return Err(format!(
            "provider project-resolution did not resolve: languageId={} providerId={} state={} reasonKind={} message={} nextAction={}",
            packet.language_id,
            packet.provider_id,
            packet.state,
            failure.reason_kind,
            failure.message,
            failure.next_action
        ));
    }
    let scope = packet
        .scope
        .ok_or_else(|| "resolved provider project-resolution response omitted scope".to_string())?;
    let expected_language_identity =
        agent_semantic_content_identity::LanguageIdV1::from(expected_language_id.as_str());
    let expected_provider_identity =
        agent_semantic_content_identity::ProviderIdV1::from(expected_provider_id.as_str());
    scope
        .validate(
            &expected_language_identity,
            &expected_provider_identity,
            &candidates.generation_digest,
        )
        .map_err(|error| format!("invalid project-resolution contract: {error}"))?;

    let mut candidates_by_prefix: std::collections::BTreeMap<
        String,
        std::collections::BTreeSet<String>,
    > = std::collections::BTreeMap::new();
    let mut candidates_by_extension: std::collections::BTreeMap<
        String,
        std::collections::BTreeSet<String>,
    > = std::collections::BTreeMap::new();
    for candidate in &candidates.paths {
        candidates_by_prefix
            .entry(".".to_string())
            .or_default()
            .insert(candidate.clone());
        let components = candidate.split('/').collect::<Vec<_>>();
        for component_count in 1..=components.len() {
            candidates_by_prefix
                .entry(components[..component_count].join("/"))
                .or_default()
                .insert(candidate.clone());
        }
        let basename = candidate.rsplit('/').next().unwrap_or(candidate);
        for (index, character) in basename.char_indices() {
            if character == '.' {
                candidates_by_extension
                    .entry(basename[index..].to_string())
                    .or_default()
                    .insert(candidate.clone());
            }
        }
    }

    let mut resolved_paths = std::collections::BTreeSet::new();
    for source_scope in &scope.source_scopes {
        if source_scope.roots.is_empty() || source_scope.extensions.is_empty() {
            return Err("project-resolution must declare roots and extensions".to_string());
        }
        match (
            source_scope.include_authority.as_str(),
            source_scope.explicit_paths.is_empty(),
        ) {
            ("manifest-explicit", true) => {
                return Err(
                    "manifest-explicit project-resolution must declare explicitPaths".to_string(),
                );
            }
            ("manifest-explicit", false) | ("package-manager", true) => {}
            (authority, false) => {
                return Err(format!(
                    "project-resolution cannot declare explicitPaths for includeAuthority={authority}"
                ));
            }
            (authority, true) => {
                return Err(format!(
                    "unsupported project-resolution includeAuthority={authority}"
                ));
            }
        }

        let mut scope_paths = std::collections::BTreeSet::new();
        for root in &source_scope.roots {
            if let Some(paths) = candidates_by_prefix.get(root) {
                scope_paths.extend(paths.iter().cloned());
            }
        }
        let mut extension_paths = std::collections::BTreeSet::new();
        for extension in &source_scope.extensions {
            if let Some(paths) = candidates_by_extension.get(extension) {
                extension_paths.extend(paths.iter().cloned());
            }
        }
        scope_paths.retain(|path| extension_paths.contains(path));
        for exclusion in &source_scope.exclusions {
            if exclusion.authority != "package-manager" {
                return Err(format!(
                    "provider-project-resolution-invalid-exclusion-authority: prefix={} authority={} expected=package-manager",
                    exclusion.prefix, exclusion.authority
                ));
            }
            if let Some(paths) = candidates_by_prefix.get(&exclusion.prefix) {
                for path in paths {
                    scope_paths.remove(path);
                }
            }
        }

        let policy_paths = candidates
            .policy_exclusions
            .iter()
            .filter(|exclusion| {
                source_scope.roots.iter().any(|root| {
                    candidates_by_prefix
                        .get(root)
                        .is_some_and(|paths| paths.contains(&exclusion.path))
                }) && source_scope
                    .extensions
                    .iter()
                    .any(|extension| exclusion.path.ends_with(extension))
            })
            .collect::<Vec<_>>();
        if source_scope.include_authority == "manifest-explicit" {
            if let Some(exclusion) = policy_paths.iter().find(|exclusion| {
                source_scope
                    .explicit_paths
                    .iter()
                    .any(|explicit_path| candidate_path_is_within(&exclusion.path, explicit_path))
            }) {
                return Err(format!(
                    "project-resolution-conflict: path={} includeAuthority=manifest-explicit excludeAuthority={} reasonKind=explicit-source-excluded",
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
        .map(|path| ProviderProjectResolutionFile {
            path,
            language_id: packet.language_id.clone(),
            provider_id: packet.provider_id.clone(),
        })
        .collect();
    Ok(ProviderProjectResolution::Supported(
        ProviderProjectResolutionPacket {
            language_id: packet.language_id,
            provider_id: packet.provider_id,
            resolution: scope,
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
    scope: Option<agent_semantic_content_identity::ProjectResolutionReceipt>,
    failure: Option<RawProviderProjectResolutionFailure>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawProviderProjectResolutionFailure {
    reason_kind: String,
    message: String,
    next_action: String,
}

fn candidate_path_is_within(candidate: &str, root: &str) -> bool {
    root == "."
        || candidate == root
        || candidate
            .strip_prefix(root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}
