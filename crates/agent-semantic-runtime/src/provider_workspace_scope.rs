//! Provider workspace-scope packet execution and parsing.

use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    ClientMethod, ClientRequest, LanguageId, ProviderId, ProviderRegistrySnapshot, ResolvedProvider,
};
use agent_semantic_client_local_cli::LocalNativeCliBackend;
use serde::Deserialize;

pub const PROVIDER_WORKSPACE_SCOPE_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-workspace-scope";

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
    let output = match LocalNativeCliBackend::new(snapshot).execute(&request) {
        Ok(output) => output,
        Err(_) => return Ok(ProviderWorkspaceScope::Unsupported),
    };
    if output.status_code != 0 {
        return Ok(ProviderWorkspaceScope::Unsupported);
    }
    provider_workspace_scope_from_stdout(output.stdout.as_ref(), provider)
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
            language_id: file
                .language_id
                .unwrap_or_else(|| language_id.clone()),
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
