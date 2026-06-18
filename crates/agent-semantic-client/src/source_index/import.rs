//! Import packet assembly for Rust SQL source-index rows.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use agent_semantic_client_core::CacheGenerationId;
use agent_semantic_client_core::ClientCacheFileHash;
use agent_semantic_client_db::{
    ClientDbSourceIndexImport, ClientDbSourceIndexOwner, ClientDbSourceIndexPath,
    ClientDbSourceIndexQueryKey, ClientDbSourceIndexSelector, ClientDbSourceIndexSource,
};
use sha2::{Digest, Sha256};

use super::config::{
    SOURCE_INDEX_FILE_BYTES_LIMIT, SOURCE_INDEX_PROVIDER_ID, SOURCE_INDEX_SCHEMA_ID,
    SOURCE_INDEX_SCHEMA_VERSION,
};
use super::model::SourceIndexScopeFile;
use super::text::{source_line_count, source_query_keys};

const SCOPE_DIR_EVIDENCE_PREFIX: &str = "@scope/dir/";
const SCOPE_REGISTRY_EVIDENCE_PATH: &str = "@scope/registry";
const SCOPE_WITNESS_SHA256: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

pub(super) fn source_index_file_hashes(
    project_root: &Path,
    files: &[SourceIndexScopeFile],
    previous_file_hashes: Option<&[ClientCacheFileHash]>,
    registry_fingerprint: &str,
    extra_scope_dirs: &BTreeSet<String>,
) -> Result<Vec<ClientCacheFileHash>, String> {
    let previous_by_path = previous_file_hashes.map(|file_hashes| {
        file_hashes
            .iter()
            .map(|file_hash| (file_hash.path.as_str(), file_hash))
            .collect::<BTreeMap<_, _>>()
    });
    let mut file_hashes = files
        .iter()
        .map(|file| source_index_file_hash(project_root, file, previous_by_path.as_ref()))
        .collect::<Result<Vec<_>, _>>()?;
    file_hashes.extend(source_scope_evidence_hashes(
        project_root,
        files,
        registry_fingerprint,
        extra_scope_dirs,
    )?);
    Ok(file_hashes)
}

pub(super) fn source_index_import_with_file_hashes(
    project_root: &Path,
    generation_id: CacheGenerationId,
    files: &[SourceIndexScopeFile],
    file_hashes: Vec<ClientCacheFileHash>,
) -> Result<ClientDbSourceIndexImport, String> {
    let file_hash_by_path = file_hashes
        .iter()
        .map(|file_hash| (file_hash.path.as_str(), file_hash))
        .collect::<BTreeMap<_, _>>();
    let mut owners = Vec::with_capacity(files.len());
    let mut selectors = Vec::with_capacity(files.len());
    for file in files {
        let relative_path = relative_project_path(project_root, &file.path);
        let Some(file_hash) = file_hash_by_path.get(relative_path.as_str()) else {
            return Err(format!("missing source index hash for {relative_path}"));
        };
        let relative_path = file_hash.path.clone();
        let text = if file_hash.byte_len <= SOURCE_INDEX_FILE_BYTES_LIMIT {
            let bytes = fs::read(&file.path).map_err(|error| {
                format!(
                    "failed to read source index file {}: {error}",
                    file.path.display()
                )
            })?;
            String::from_utf8(bytes).unwrap_or_default()
        } else {
            String::new()
        };
        let line_count = source_line_count(&text);
        let query_keys = source_query_keys(&relative_path, &text);
        let owner_path = ClientDbSourceIndexPath::from(relative_path.clone());
        owners.push(ClientDbSourceIndexOwner {
            owner_path: owner_path.clone(),
            language_id: Some(file.language_id.clone()),
            provider_id: Some(file.provider_id.clone()),
            source_kind: ClientDbSourceIndexSource::from("file"),
            line_count: Some(line_count),
            query_keys: query_keys
                .iter()
                .cloned()
                .map(ClientDbSourceIndexQueryKey::from)
                .collect(),
        });
        selectors.push(ClientDbSourceIndexSelector {
            owner_path,
            selector_id: format!("{}://{relative_path}#file", file.language_id.as_str()),
            symbol: file
                .path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string),
            kind: Some("file".to_string()),
            start_line: 1,
            end_line: line_count.max(1),
            source: ClientDbSourceIndexSource::from(SOURCE_INDEX_PROVIDER_ID),
            query_keys: query_keys
                .into_iter()
                .map(ClientDbSourceIndexQueryKey::from)
                .collect(),
        });
    }
    Ok(ClientDbSourceIndexImport {
        generation_id,
        project_root: project_root.to_path_buf(),
        schema_id: SOURCE_INDEX_SCHEMA_ID.into(),
        schema_version: SOURCE_INDEX_SCHEMA_VERSION.into(),
        file_hashes,
        owners,
        selectors,
    })
}

fn source_scope_evidence_hashes(
    project_root: &Path,
    files: &[SourceIndexScopeFile],
    registry_fingerprint: &str,
    extra_scope_dirs: &BTreeSet<String>,
) -> Result<Vec<ClientCacheFileHash>, String> {
    let mut evidence = Vec::new();
    evidence.push(ClientCacheFileHash {
        path: SCOPE_REGISTRY_EVIDENCE_PATH.to_string(),
        sha256: format!("{:x}", Sha256::digest(registry_fingerprint.as_bytes())),
        byte_len: registry_fingerprint.len().min(u64::MAX as usize) as u64,
        mtime_ms: 0,
    });
    let mut scope_dirs = source_scope_dirs(project_root, files);
    scope_dirs.extend(extra_scope_dirs.iter().cloned());
    for relative_dir in scope_dirs {
        let dir_path = if relative_dir == "." {
            project_root.to_path_buf()
        } else {
            project_root.join(&relative_dir)
        };
        let metadata = fs::metadata(&dir_path).map_err(|error| {
            format!(
                "failed to read source index dir metadata {}: {error}",
                dir_path.display()
            )
        })?;
        let mtime_ms = metadata_mtime_ms(&metadata, &dir_path)?;
        evidence.push(ClientCacheFileHash {
            path: format!("{SCOPE_DIR_EVIDENCE_PREFIX}{relative_dir}"),
            sha256: SCOPE_WITNESS_SHA256.to_string(),
            byte_len: metadata.len(),
            mtime_ms,
        });
    }
    Ok(evidence)
}

fn source_scope_dirs(project_root: &Path, files: &[SourceIndexScopeFile]) -> BTreeSet<String> {
    let mut dirs = BTreeSet::new();
    dirs.insert(".".to_string());
    for file in files {
        let relative_path = relative_project_path(project_root, &file.path);
        let path = Path::new(&relative_path);
        let mut parent = path.parent();
        while let Some(dir) = parent {
            let value = dir.to_string_lossy();
            if value.is_empty() {
                dirs.insert(".".to_string());
                break;
            }
            dirs.insert(value.to_string());
            parent = dir.parent();
        }
    }
    dirs
}

fn source_index_file_hash(
    project_root: &Path,
    file: &SourceIndexScopeFile,
    previous_by_path: Option<&BTreeMap<&str, &ClientCacheFileHash>>,
) -> Result<ClientCacheFileHash, String> {
    let metadata = fs::metadata(&file.path).map_err(|error| {
        format!(
            "failed to read source index file metadata {}: {error}",
            file.path.display()
        )
    })?;
    let mtime_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .ok_or_else(|| {
            format!(
                "failed to read source index file mtime {}",
                file.path.display()
            )
        })?;
    let relative_path = relative_project_path(project_root, &file.path);
    if let Some(previous) = previous_by_path.and_then(|hashes| hashes.get(relative_path.as_str()))
        && previous.byte_len == metadata.len()
        && previous.mtime_ms == mtime_ms
    {
        return Ok((*previous).clone());
    }
    let bytes = fs::read(&file.path).map_err(|error| {
        format!(
            "failed to read source index file {}: {error}",
            file.path.display()
        )
    })?;
    Ok(ClientCacheFileHash {
        path: relative_path,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        byte_len: metadata.len(),
        mtime_ms,
    })
}

fn metadata_mtime_ms(metadata: &fs::Metadata, path: &Path) -> Result<u64, String> {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .ok_or_else(|| {
            format!(
                "failed to read source index metadata mtime {}",
                path.display()
            )
        })
}

fn relative_project_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
