//! Provider workspace-scope packet execution and parsing.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_semantic_client_core::{
    ClientMethod, ClientRequest, LanguageId, ProviderId, ProviderRegistrySnapshot,
    ResolvedProvider, project_child_path, provider_ignores_path, provider_supports_source_file,
    scoped_child_path,
};
use agent_semantic_provider_transport::ProviderProcessLimits;
use serde::Deserialize;

use crate::LocalNativeCliBackend;

pub const PROVIDER_WORKSPACE_SCOPE_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-workspace-scope";
const WORKSPACE_SCOPE_PROVIDER_TIMEOUT_MS: u64 = 750;
const WORKSPACE_SCOPE_MAX_STDOUT_BYTES: usize = 1024 * 1024;
const WORKSPACE_SCOPE_MAX_STDERR_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderWorkspaceScope {
    Supported(ProviderWorkspaceScopePacket),
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderWorkspaceScopePacket {
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub files: Vec<ProviderWorkspaceScopeFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderWorkspaceScopeFile {
    pub path: String,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderWorkspaceScopeFiles {
    Supported(Vec<ProviderWorkspaceScopePathFile>),
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderWorkspaceScopePathFile {
    pub path: PathBuf,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
}

pub fn collect_provider_source_scope_files(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    limit: usize,
) -> Result<Vec<ProviderWorkspaceScopePathFile>, String> {
    if snapshot.providers.is_empty() {
        return Err(missing_provider_scope_message(project_root));
    }
    let mut files = BTreeMap::new();
    for provider in &snapshot.providers {
        collect_provider_source_scope_files_for_provider(
            project_root,
            provider,
            limit,
            &mut files,
        )?;
        if files.len() >= limit {
            break;
        }
    }
    if files.is_empty() {
        return Err(format!(
            "missing provider source-scope facts: activated providers exposed no source/config files for {}; run `asp install plugin --codex .` or refresh the language provider workspace facts",
            project_root.display()
        ));
    }
    Ok(files.into_values().take(limit).collect())
}

pub fn provider_workspace_scope(
    project_root: &Path,
    provider: &ResolvedProvider,
    package_root: &str,
) -> Result<ProviderWorkspaceScope, String> {
    let request = ClientRequest::new(ClientMethod::Search, project_root.to_path_buf())
        .with_language(provider.language_id.clone())
        .with_forwarded_args(vec![
            "workspace-scope".to_string(),
            "--json".to_string(),
            package_root.to_string(),
        ]);
    let snapshot = ProviderRegistrySnapshot {
        activation_path: PathBuf::new(),
        providers: vec![provider.clone()],
    };
    let output = match LocalNativeCliBackend::new(snapshot)
        .execute_with_limits(&request, workspace_scope_provider_limits())
    {
        Ok(output) => output,
        Err(_) => return Ok(ProviderWorkspaceScope::Unsupported),
    };
    if output.status_code != 0 {
        return Ok(ProviderWorkspaceScope::Unsupported);
    }
    provider_workspace_scope_from_stdout(output.stdout.as_ref(), provider)
}

fn workspace_scope_provider_limits() -> ProviderProcessLimits {
    ProviderProcessLimits {
        timeout: Some(Duration::from_millis(WORKSPACE_SCOPE_PROVIDER_TIMEOUT_MS)),
        max_stdout_bytes: Some(WORKSPACE_SCOPE_MAX_STDOUT_BYTES),
        max_stderr_bytes: Some(WORKSPACE_SCOPE_MAX_STDERR_BYTES),
    }
}

pub fn provider_workspace_scope_files(
    project_root: &Path,
    provider: &ResolvedProvider,
    package_root: &str,
    package_root_path: &Path,
) -> Result<ProviderWorkspaceScopeFiles, String> {
    let ProviderWorkspaceScope::Supported(packet) =
        provider_workspace_scope(project_root, provider, package_root)?
    else {
        return Ok(ProviderWorkspaceScopeFiles::Unsupported);
    };
    Ok(ProviderWorkspaceScopeFiles::Supported(
        provider_workspace_scope_files_from_packet(
            project_root,
            provider,
            package_root_path,
            packet,
        ),
    ))
}

#[must_use]
pub fn provider_workspace_scope_files_from_packet(
    project_root: &Path,
    provider: &ResolvedProvider,
    package_root_path: &Path,
    packet: ProviderWorkspaceScopePacket,
) -> Vec<ProviderWorkspaceScopePathFile> {
    packet
        .files
        .into_iter()
        .filter_map(|file| {
            let path = scoped_child_path(package_root_path, &file.path)?;
            (path.is_file() && !provider_ignores_path(project_root, provider, &path)).then_some(
                ProviderWorkspaceScopePathFile {
                    path,
                    language_id: file.language_id,
                    provider_id: file.provider_id,
                },
            )
        })
        .collect()
}

pub fn provider_workspace_scope_from_stdout(
    stdout: &[u8],
    provider: &ResolvedProvider,
) -> Result<ProviderWorkspaceScope, String> {
    let stdout = String::from_utf8_lossy(stdout);
    let Some(packet) = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find_map(parse_workspace_scope_packet)
    else {
        return Ok(ProviderWorkspaceScope::Unsupported);
    };
    if packet
        .schema_id
        .as_deref()
        .is_some_and(|schema_id| schema_id != PROVIDER_WORKSPACE_SCOPE_SCHEMA_ID)
    {
        return Ok(ProviderWorkspaceScope::Unsupported);
    }
    if packet.status.as_deref() == Some("missing-anchor") {
        return Ok(ProviderWorkspaceScope::Supported(
            ProviderWorkspaceScopePacket {
                language_id: provider.language_id.clone(),
                provider_id: provider.provider_id.clone(),
                files: Vec::new(),
            },
        ));
    }
    if packet.status.is_none() && packet.files.is_empty() {
        return Ok(ProviderWorkspaceScope::Unsupported);
    }
    let language_id = packet
        .language_id
        .unwrap_or_else(|| provider.language_id.clone());
    let provider_id = packet
        .provider_id
        .unwrap_or_else(|| provider.provider_id.clone());
    let files = packet
        .files
        .into_iter()
        .map(|file| ProviderWorkspaceScopeFile {
            path: file.path,
            language_id: file.language_id.unwrap_or_else(|| language_id.clone()),
            provider_id: file.provider_id.unwrap_or_else(|| provider_id.clone()),
        })
        .collect();
    Ok(ProviderWorkspaceScope::Supported(
        ProviderWorkspaceScopePacket {
            language_id,
            provider_id,
            files,
        },
    ))
}

fn parse_workspace_scope_packet(line: &str) -> Option<RawProviderWorkspaceScopePacket> {
    serde_json::from_str::<RawProviderWorkspaceScopePacket>(line).ok()
}

fn collect_provider_source_scope_files_for_provider(
    project_root: &Path,
    provider: &ResolvedProvider,
    limit: usize,
    files: &mut BTreeMap<PathBuf, ProviderWorkspaceScopePathFile>,
) -> Result<(), String> {
    let package_roots = if provider.package_roots.is_empty() {
        vec![".".to_string()]
    } else {
        provider.package_roots.clone()
    };
    let mut workspace_scope_supported = false;
    for package_root in package_roots {
        if files.len() >= limit {
            break;
        }
        let Some(package_root_path) = project_child_path(project_root, &package_root) else {
            continue;
        };
        match provider_workspace_scope_files(
            project_root,
            provider,
            &package_root,
            &package_root_path,
        )? {
            ProviderWorkspaceScopeFiles::Supported(provider_files) => {
                workspace_scope_supported = true;
                insert_provider_scope_files(files, provider_files, limit);
            }
            ProviderWorkspaceScopeFiles::Unsupported => {
                if !workspace_scope_supported {
                    collect_provider_manifest_scope_files(
                        project_root,
                        provider,
                        &package_root_path,
                        limit,
                        files,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn collect_provider_manifest_scope_files(
    project_root: &Path,
    provider: &ResolvedProvider,
    package_root: &Path,
    limit: usize,
    files: &mut BTreeMap<PathBuf, ProviderWorkspaceScopePathFile>,
) -> Result<(), String> {
    collect_provider_config_files(project_root, provider, package_root, limit, files);
    for source_root in &provider.source_roots {
        if files.len() >= limit {
            break;
        }
        let Some(source_root_path) = scoped_child_path(package_root, source_root) else {
            continue;
        };
        collect_provider_source_files(project_root, provider, &source_root_path, limit, files)?;
    }
    Ok(())
}

fn collect_provider_config_files(
    project_root: &Path,
    provider: &ResolvedProvider,
    package_root: &Path,
    limit: usize,
    files: &mut BTreeMap<PathBuf, ProviderWorkspaceScopePathFile>,
) {
    for config_file in &provider.config_files {
        if files.len() >= limit {
            return;
        }
        let Some(path) = scoped_child_path(package_root, config_file) else {
            continue;
        };
        if path.is_file() && !provider_ignores_path(project_root, provider, &path) {
            insert_provider_file(files, provider, path);
        }
    }
}

fn collect_provider_source_files(
    project_root: &Path,
    provider: &ResolvedProvider,
    dir: &Path,
    limit: usize,
    files: &mut BTreeMap<PathBuf, ProviderWorkspaceScopePathFile>,
) -> Result<(), String> {
    if files.len() >= limit || !dir.is_dir() || provider_ignores_path(project_root, provider, dir) {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)
        .map_err(|error| {
            format!(
                "failed to read provider scope dir {}: {error}",
                dir.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "failed to read provider scope entry under {}: {error}",
                dir.display()
            )
        })?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        if files.len() >= limit {
            break;
        }
        let path = entry.path();
        if provider_ignores_path(project_root, provider, &path) {
            continue;
        }
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "failed to inspect provider scope path {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_dir() {
            collect_provider_source_files(project_root, provider, &path, limit, files)?;
        } else if file_type.is_file() && provider_supports_source_file(provider, &path) {
            insert_provider_file(files, provider, path);
        }
    }
    Ok(())
}

fn insert_provider_scope_files(
    files: &mut BTreeMap<PathBuf, ProviderWorkspaceScopePathFile>,
    provider_files: Vec<ProviderWorkspaceScopePathFile>,
    limit: usize,
) {
    for file in provider_files {
        files.entry(file.path.clone()).or_insert(file);
        if files.len() >= limit {
            break;
        }
    }
}

fn insert_provider_file(
    files: &mut BTreeMap<PathBuf, ProviderWorkspaceScopePathFile>,
    provider: &ResolvedProvider,
    path: PathBuf,
) {
    files
        .entry(path.clone())
        .or_insert_with(|| ProviderWorkspaceScopePathFile {
            path,
            language_id: provider.language_id.clone(),
            provider_id: provider.provider_id.clone(),
        });
}

fn missing_provider_scope_message(project_root: &Path) -> String {
    format!(
        "missing provider source-scope facts: no activated language providers for {}; run `asp install plugin --codex .` so language harnesses can expose workspace coverage to the source index",
        project_root.display()
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawProviderWorkspaceScopePacket {
    #[serde(default)]
    schema_id: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    language_id: Option<LanguageId>,
    #[serde(default)]
    provider_id: Option<ProviderId>,
    #[serde(default)]
    files: Vec<RawProviderWorkspaceScopeFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawProviderWorkspaceScopeFile {
    path: String,
    #[serde(default)]
    language_id: Option<LanguageId>,
    #[serde(default)]
    provider_id: Option<ProviderId>,
}

#[cfg(test)]
#[path = "../tests/unit/provider_workspace_scope.rs"]
mod provider_workspace_scope_tests;
