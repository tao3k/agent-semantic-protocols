//! DB-owned source-index import packet assembly.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use agent_semantic_client_core::ClientCacheFileHash;
use sha2::{Digest, Sha256};

use super::text::{source_line_count, source_query_keys};
use super::types::{
    ClientDbSourceIndexImport, ClientDbSourceIndexImportAssemblyRequest,
    ClientDbSourceIndexImportFile, ClientDbSourceIndexImportRequest, ClientDbSourceIndexOwner,
    ClientDbSourceIndexPath, ClientDbSourceIndexQueryKey, ClientDbSourceIndexScopeFile,
    ClientDbSourceIndexSelector, ClientDbSourceIndexSource,
};
use super::types::{
    client_db_source_index_registry_evidence_hash, client_db_source_index_scope_dir_evidence_hash,
};

/// Build source-index file hashes and import rows from collected workspace
/// files. Raw source text is used only transiently for owner/query projection.
pub fn assemble_source_index_import(
    request: ClientDbSourceIndexImportAssemblyRequest,
) -> Result<ClientDbSourceIndexImport, String> {
    let file_hashes = source_index_file_hashes(
        &request.project_root,
        &request.files,
        request.previous_file_hashes.as_deref(),
        &request.registry_fingerprint,
        request.extra_scope_dirs.iter().map(String::as_str),
    )?;
    source_index_import_with_file_hashes(request, file_hashes)
}

/// Return source-index file and scope evidence hashes without assembling rows.
pub fn source_index_file_hashes<'a>(
    project_root: &Path,
    files: &[ClientDbSourceIndexScopeFile],
    previous_file_hashes: Option<&[ClientCacheFileHash]>,
    registry_fingerprint: &str,
    extra_scope_dirs: impl IntoIterator<Item = &'a str>,
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

/// Build a source-index import from precomputed file hashes.
pub fn source_index_import_with_file_hashes(
    request: ClientDbSourceIndexImportAssemblyRequest,
    file_hashes: Vec<ClientCacheFileHash>,
) -> Result<ClientDbSourceIndexImport, String> {
    let file_hash_by_path = file_hashes
        .iter()
        .map(|file_hash| (file_hash.path.as_str(), file_hash))
        .collect::<BTreeMap<_, _>>();
    let mut import_files = Vec::with_capacity(request.files.len());
    for file in &request.files {
        let relative_path = source_index_relative_path(&request.project_root, &file.path);
        let Some(file_hash) = file_hash_by_path.get(relative_path.as_str()) else {
            return Err(format!("missing source index hash for {relative_path}"));
        };
        let relative_path = file_hash.path.clone();
        let text = if file_hash.byte_len <= request.file_text_bytes_limit {
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
        generation_id: request.generation_id,
        project_root: request.project_root,
        schema_id: request.schema_id,
        schema_version: request.schema_version,
        selector_source: request.selector_source,
        file_hashes,
        files: import_files,
    })
}

/// Build the DB-owned source-index import packet from collected file facts.
pub fn build_source_index_import(
    request: ClientDbSourceIndexImportRequest,
) -> Result<ClientDbSourceIndexImport, String> {
    let file_hash_by_path = request
        .file_hashes
        .iter()
        .map(|file_hash| (file_hash.path.as_str(), file_hash))
        .collect::<BTreeMap<_, _>>();
    let mut owners = Vec::with_capacity(request.files.len());
    let mut selectors = Vec::with_capacity(request.files.len());
    for file in &request.files {
        let Some(file_hash) = file_hash_by_path.get(file.relative_path.as_str()) else {
            return Err(format!(
                "missing source index hash for {}",
                file.relative_path
            ));
        };
        let relative_path = file_hash.path.clone();
        let line_count = source_line_count(&file.text);
        let query_keys = source_query_keys(&relative_path, &file.text);
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
            symbol: file_symbol(&relative_path),
            kind: Some("file".to_string()),
            start_line: 1,
            end_line: line_count.max(1),
            source: request.selector_source.clone(),
            query_keys: query_keys
                .into_iter()
                .map(ClientDbSourceIndexQueryKey::from)
                .collect(),
        });
    }
    Ok(ClientDbSourceIndexImport {
        generation_id: request.generation_id,
        project_root: request.project_root,
        schema_id: request.schema_id,
        schema_version: request.schema_version,
        file_hashes: request.file_hashes,
        owners,
        selectors,
    })
}

fn file_symbol(relative_path: &str) -> Option<String> {
    let file_name = relative_path.rsplit('/').next()?;
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _extension)| stem);
    if stem.is_empty() {
        None
    } else {
        Some(stem.to_string())
    }
}

/// Return the slash-normalized project-relative path used by source-index rows.
#[must_use]
pub fn source_index_relative_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Return slash-normalized scope directories covered by source-index files.
#[must_use]
pub fn source_index_scope_dirs(
    project_root: &Path,
    files: &[ClientDbSourceIndexScopeFile],
) -> BTreeSet<String> {
    let mut dirs = BTreeSet::new();
    dirs.insert(".".to_string());
    for file in files {
        let relative_path = source_index_relative_path(project_root, &file.path);
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

fn source_scope_evidence_hashes<'a>(
    project_root: &Path,
    files: &[ClientDbSourceIndexScopeFile],
    registry_fingerprint: &str,
    extra_scope_dirs: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<ClientCacheFileHash>, String> {
    let mut evidence = Vec::new();
    evidence.push(client_db_source_index_registry_evidence_hash(
        registry_fingerprint,
    ));
    let mut scope_dirs = source_index_scope_dirs(project_root, files);
    scope_dirs.extend(extra_scope_dirs.into_iter().map(ToString::to_string));
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
        evidence.push(client_db_source_index_scope_dir_evidence_hash(
            &relative_dir,
            metadata.len(),
            mtime_ms,
        ));
    }
    Ok(evidence)
}

fn source_index_file_hash(
    project_root: &Path,
    file: &ClientDbSourceIndexScopeFile,
    previous_by_path: Option<&BTreeMap<&str, &ClientCacheFileHash>>,
) -> Result<ClientCacheFileHash, String> {
    let metadata = fs::metadata(&file.path).map_err(|error| {
        format!(
            "failed to read source index file metadata {}: {error}",
            file.path.display()
        )
    })?;
    let mtime_ms = metadata_mtime_ms(&metadata, &file.path)?;
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
