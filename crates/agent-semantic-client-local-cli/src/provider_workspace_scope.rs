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

pub fn provider_workspace_scope(
    provider: &ResolvedProvider,
    project_root: &Path,
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
    ProviderProcessLimits::new(
        Some(Duration::from_millis(WORKSPACE_SCOPE_PROVIDER_TIMEOUT_MS)),
        Some(WORKSPACE_SCOPE_MAX_STDOUT_BYTES),
        Some(WORKSPACE_SCOPE_MAX_STDERR_BYTES),
        Some(1024 * 1024 * 1024),
    )
}

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
        provider_workspace_scope_files_from_packet(
            project_root,
            package_root_path,
            packet,
        ),
    ))
}

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
    if packet.schema_id.as_deref().is_some_and(|schema_id| {
        schema_id != PROVIDER_WORKSPACE_SCOPE_SCHEMA_ID
            && schema_id != "agent.semantic-protocols.semantic-workspace-scope"
    }) {
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
#[cfg(test)]
#[path = "../tests/unit/provider_workspace_scope.rs"]
mod tests;
