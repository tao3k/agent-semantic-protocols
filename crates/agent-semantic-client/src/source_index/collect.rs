//! Provider-scope collection for Rust SQL source-index refresh.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use agent_semantic_client_core::{
    ClientMethod, ClientRequest, LanguageId, ProviderId, ProviderRegistrySnapshot, ResolvedProvider,
};
use agent_semantic_client_local_cli::LocalNativeCliBackend;
use serde::Deserialize;

use super::config::SOURCE_INDEX_FILE_LIMIT;
use super::model::SourceIndexScopeFile;

pub(super) fn collect_source_index_files(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
) -> Result<Vec<SourceIndexScopeFile>, String> {
    if snapshot.providers.is_empty() {
        return Err(missing_provider_scope_message(project_root));
    }
    let files = collect_source_index_file_map(project_root, snapshot)?;
    if files.is_empty() {
        return Err(format!(
            "missing provider source-scope facts: activated providers exposed no source/config files for {}; run `asp install plugin --codex .` or refresh the language provider workspace facts",
            project_root.display()
        ));
    }
    Ok(files.into_values().take(SOURCE_INDEX_FILE_LIMIT).collect())
}

fn collect_source_index_file_map(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
) -> Result<BTreeMap<PathBuf, SourceIndexScopeFile>, String> {
    let mut files = BTreeMap::new();
    for provider in &snapshot.providers {
        collect_provider_source_index_files(project_root, provider, &mut files)?;
        if files.len() >= SOURCE_INDEX_FILE_LIMIT {
            break;
        }
    }
    Ok(files)
}

fn collect_provider_source_index_files(
    project_root: &Path,
    provider: &ResolvedProvider,
    files: &mut BTreeMap<PathBuf, SourceIndexScopeFile>,
) -> Result<(), String> {
    match collect_provider_workspace_scope_files(project_root, provider)? {
        WorkspaceScopeCollection::Supported(provider_files) => {
            insert_source_index_scope_files(files, provider_files);
        }
        WorkspaceScopeCollection::Unsupported => {
            collect_provider_scope_files(project_root, provider, files)?;
        }
    }
    Ok(())
}

fn insert_source_index_scope_files(
    files: &mut BTreeMap<PathBuf, SourceIndexScopeFile>,
    provider_files: Vec<SourceIndexScopeFile>,
) {
    for file in provider_files {
        files.entry(file.path.clone()).or_insert(file);
        if files.len() >= SOURCE_INDEX_FILE_LIMIT {
            break;
        }
    }
}

enum WorkspaceScopeCollection {
    Supported(Vec<SourceIndexScopeFile>),
    Unsupported,
}

fn collect_provider_workspace_scope_files(
    project_root: &Path,
    provider: &ResolvedProvider,
) -> Result<WorkspaceScopeCollection, String> {
    let package_roots = if provider.package_roots.is_empty() {
        vec![".".to_string()]
    } else {
        provider.package_roots.clone()
    };
    let mut files = BTreeMap::new();
    let mut supported = false;
    for package_root in package_roots {
        if files.len() >= SOURCE_INDEX_FILE_LIMIT {
            break;
        }
        let Some(package_root_path) = project_child_path(project_root, &package_root) else {
            continue;
        };
        let Some(scope_files) = provider_workspace_scope_files(
            project_root,
            provider,
            &package_root,
            &package_root_path,
        )?
        else {
            continue;
        };
        supported = true;
        for file in scope_files {
            files.entry(file.path.clone()).or_insert(file);
            if files.len() >= SOURCE_INDEX_FILE_LIMIT {
                break;
            }
        }
    }
    if supported {
        Ok(WorkspaceScopeCollection::Supported(
            files.into_values().take(SOURCE_INDEX_FILE_LIMIT).collect(),
        ))
    } else {
        Ok(WorkspaceScopeCollection::Unsupported)
    }
}

fn provider_workspace_scope_files(
    project_root: &Path,
    provider: &ResolvedProvider,
    package_root: &str,
    package_root_path: &Path,
) -> Result<Option<Vec<SourceIndexScopeFile>>, String> {
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
    let output = LocalNativeCliBackend::new(snapshot).execute(&request)?;
    if output.status_code != 0 {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(output.stdout.as_ref());
    let Some(packet) = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find_map(parse_workspace_scope_packet)
    else {
        return Ok(None);
    };
    if packet
        .schema_id
        .as_deref()
        .is_some_and(|schema_id| schema_id != "agent.semantic-protocols.semantic-workspace-scope")
    {
        return Ok(None);
    }
    if packet.status.as_deref() == Some("missing-anchor") {
        return Ok(Some(Vec::new()));
    }
    if packet.status.is_none() && packet.files.is_empty() {
        return Ok(None);
    }
    let language_id = packet
        .language_id
        .unwrap_or_else(|| provider.language_id.clone());
    let provider_id = packet
        .provider_id
        .unwrap_or_else(|| provider.provider_id.clone());
    let mut files = Vec::new();
    for file in packet.files {
        let Some(path) = scoped_child_path(package_root_path, &file.path) else {
            continue;
        };
        if path.is_file() && !provider_ignores_path(project_root, provider, &path) {
            files.push(SourceIndexScopeFile {
                path,
                language_id: file
                    .language_id
                    .clone()
                    .unwrap_or_else(|| language_id.clone()),
                provider_id: file
                    .provider_id
                    .clone()
                    .unwrap_or_else(|| provider_id.clone()),
            });
        }
    }
    Ok(Some(files))
}

fn parse_workspace_scope_packet(line: &str) -> Option<ProviderWorkspaceScopePacket> {
    serde_json::from_str::<ProviderWorkspaceScopePacket>(line).ok()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderWorkspaceScopePacket {
    #[serde(default)]
    schema_id: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    language_id: Option<LanguageId>,
    #[serde(default)]
    provider_id: Option<ProviderId>,
    #[serde(default)]
    files: Vec<ProviderWorkspaceScopeFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderWorkspaceScopeFile {
    path: String,
    #[serde(default)]
    language_id: Option<LanguageId>,
    #[serde(default)]
    provider_id: Option<ProviderId>,
}

fn collect_provider_scope_files(
    project_root: &Path,
    provider: &ResolvedProvider,
    files: &mut BTreeMap<PathBuf, SourceIndexScopeFile>,
) -> Result<(), String> {
    let package_roots = if provider.package_roots.is_empty() {
        vec![".".to_string()]
    } else {
        provider.package_roots.clone()
    };
    for package_root in package_roots {
        let Some(package_root_path) = project_child_path(project_root, &package_root) else {
            continue;
        };
        collect_provider_config_files(project_root, provider, &package_root_path, files);
        for source_root in &provider.source_roots {
            if files.len() >= SOURCE_INDEX_FILE_LIMIT {
                break;
            }
            let Some(source_root_path) = scoped_child_path(&package_root_path, source_root) else {
                continue;
            };
            collect_provider_source_files(project_root, provider, &source_root_path, files)?;
        }
        if files.len() >= SOURCE_INDEX_FILE_LIMIT {
            break;
        }
    }
    Ok(())
}

fn collect_provider_config_files(
    project_root: &Path,
    provider: &ResolvedProvider,
    package_root: &Path,
    files: &mut BTreeMap<PathBuf, SourceIndexScopeFile>,
) {
    for config_file in &provider.config_files {
        if files.len() >= SOURCE_INDEX_FILE_LIMIT {
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
    files: &mut BTreeMap<PathBuf, SourceIndexScopeFile>,
) -> Result<(), String> {
    if files.len() >= SOURCE_INDEX_FILE_LIMIT
        || !dir.is_dir()
        || provider_ignores_path(project_root, provider, dir)
    {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)
        .map_err(|error| format!("failed to read source index dir {}: {error}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "failed to read source index entry under {}: {error}",
                dir.display()
            )
        })?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        if files.len() >= SOURCE_INDEX_FILE_LIMIT {
            break;
        }
        let path = entry.path();
        if provider_ignores_path(project_root, provider, &path) {
            continue;
        }
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "failed to inspect source index path {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_dir() {
            collect_provider_source_files(project_root, provider, &path, files)?;
        } else if file_type.is_file() && provider_supports_source_file(provider, &path) {
            insert_provider_file(files, provider, path);
        }
    }
    Ok(())
}

fn insert_provider_file(
    files: &mut BTreeMap<PathBuf, SourceIndexScopeFile>,
    provider: &ResolvedProvider,
    path: PathBuf,
) {
    files
        .entry(path.clone())
        .or_insert_with(|| SourceIndexScopeFile {
            path,
            language_id: provider.language_id.clone(),
            provider_id: provider.provider_id.clone(),
        });
}

fn provider_supports_source_file(provider: &ResolvedProvider, path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    provider.source_extensions.iter().any(|candidate| {
        candidate
            .trim_start_matches('.')
            .eq_ignore_ascii_case(extension)
    })
}

fn provider_ignores_path(project_root: &Path, provider: &ResolvedProvider, path: &Path) -> bool {
    let relative = relative_project_path(project_root, path);
    provider.ignored_path_prefixes.iter().any(|prefix| {
        let prefix = normalize_project_path(prefix);
        relative == prefix || relative.starts_with(&format!("{prefix}/"))
    })
}

fn project_child_path(project_root: &Path, path: &str) -> Option<PathBuf> {
    if path == "." || path.is_empty() {
        return Some(project_root.to_path_buf());
    }
    scoped_child_path(project_root, path)
}

fn scoped_child_path(root: &Path, path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return None;
    }
    Some(root.join(path))
}

fn relative_project_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

fn normalize_project_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

fn missing_provider_scope_message(project_root: &Path) -> String {
    format!(
        "missing provider source-scope facts: no activated language providers for {}; run `asp install plugin --codex .` so language harnesses can expose workspace coverage to the Rust SQL source index",
        project_root.display()
    )
}
