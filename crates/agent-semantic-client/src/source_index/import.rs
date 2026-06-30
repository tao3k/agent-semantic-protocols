//! Import packet assembly for Rust SQL source-index rows.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use agent_semantic_client_core::CacheGenerationId;
use agent_semantic_client_core::ClientCacheFileHash;
use agent_semantic_client_db::{
    CLIENT_DB_SOURCE_INDEX_SCOPE_DIR_EVIDENCE_PREFIX,
    CLIENT_DB_SOURCE_INDEX_SCOPE_REGISTRY_EVIDENCE_PATH,
    CLIENT_DB_SOURCE_INDEX_SCOPE_WITNESS_SHA256, ClientDbSourceIndexImport,
    ClientDbSourceIndexImportFile, ClientDbSourceIndexImportRequest, ClientDbSourceIndexSource,
    build_source_index_import, source_index_relative_path, source_index_scope_dirs,
};
use sha2::{Digest, Sha256};

use super::config::{
    SOURCE_INDEX_FILE_BYTES_LIMIT, SOURCE_INDEX_PROVIDER_ID, SOURCE_INDEX_SCHEMA_ID,
    SOURCE_INDEX_SCHEMA_VERSION,
};
use super::model::SourceIndexScopeFile;

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
    let mut import_files = Vec::with_capacity(files.len());
    for file in files {
        let relative_path = source_index_relative_path(project_root, &file.path);
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
        import_files.push(ClientDbSourceIndexImportFile {
            relative_path,
            language_id: file.language_id.clone(),
            provider_id: file.provider_id.clone(),
            text,
        });
    }
    build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id,
        project_root: project_root.to_path_buf(),
        schema_id: SOURCE_INDEX_SCHEMA_ID.into(),
        schema_version: SOURCE_INDEX_SCHEMA_VERSION.into(),
        selector_source: ClientDbSourceIndexSource::from(SOURCE_INDEX_PROVIDER_ID),
        file_hashes,
        files: import_files,
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
        path: CLIENT_DB_SOURCE_INDEX_SCOPE_REGISTRY_EVIDENCE_PATH.to_string(),
        sha256: format!("{:x}", Sha256::digest(registry_fingerprint.as_bytes())),
        byte_len: registry_fingerprint.len().min(u64::MAX as usize) as u64,
        mtime_ms: 0,
    });
    let mut scope_dirs = source_index_scope_dirs(project_root, files);
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
            path: format!("{CLIENT_DB_SOURCE_INDEX_SCOPE_DIR_EVIDENCE_PREFIX}{relative_dir}"),
            sha256: CLIENT_DB_SOURCE_INDEX_SCOPE_WITNESS_SHA256.to_string(),
            byte_len: metadata.len(),
            mtime_ms,
        });
    }
    Ok(evidence)
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
    let relative_path = source_index_relative_path(project_root, &file.path);
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
