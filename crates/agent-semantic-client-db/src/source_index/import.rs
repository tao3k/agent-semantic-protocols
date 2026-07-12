//! DB-owned source-index import packet assembly.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use agent_semantic_client_core::{ClientCacheFileHash, SemanticSchemaId, SemanticSchemaVersion};
use sha2::{Digest, Sha256};

use super::language_projection::{
    ClientDbLanguageProjectionImportRequest, language_projection_source_index_rows,
};
use super::text::{source_line_count, source_query_keys};
use super::types::client_db_source_index_registry_evidence_hash;
use super::types::{
    CLIENT_DB_SOURCE_INDEX_SCHEMA_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION,
    ClientDbSourceIndexImport, ClientDbSourceIndexImportAssemblyRequest,
    ClientDbSourceIndexImportFile, ClientDbSourceIndexImportRequest, ClientDbSourceIndexOwner,
    ClientDbSourceIndexPath, ClientDbSourceIndexQueryKey, ClientDbSourceIndexScopeFile,
    ClientDbSourceIndexSelector, ClientDbSourceIndexSource,
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

/// Import parser-owned language projection rows without projecting raw source text.
pub fn source_index_import_from_language_projection(
    request: ClientDbLanguageProjectionImportRequest,
) -> Result<ClientDbSourceIndexImport, String> {
    let rows = language_projection_source_index_rows(&request.projection, &request.project_root)?;
    let file_hashes = source_index_file_hashes(
        &request.project_root,
        &rows.scope_files,
        request.previous_file_hashes.as_deref(),
        &request.registry_fingerprint,
        std::iter::empty(),
    )?;
    Ok(ClientDbSourceIndexImport {
        generation_id: request.generation_id,
        project_root: request.project_root,
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        file_hashes,
        owners: rows.owners,
        selectors: rows.selectors,
    })
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
    let force_content_hash =
        previous_file_hashes.is_some() && source_index_tracked_worktree_is_dirty(project_root);
    let mut file_hashes = files
        .iter()
        .map(|file| {
            source_index_file_hash(
                project_root,
                file,
                previous_by_path.as_ref(),
                force_content_hash,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let _ = extra_scope_dirs;
    file_hashes.extend(source_scope_evidence_hashes(registry_fingerprint));
    Ok(file_hashes)
}

/// Build a source-index import from precomputed file hashes.
pub fn source_index_import_with_file_hashes(
    request: ClientDbSourceIndexImportAssemblyRequest,
    file_hashes: Vec<ClientCacheFileHash>,
) -> Result<ClientDbSourceIndexImport, String> {
    let cold_assembly_started = std::time::Instant::now();
    let file_hash_by_path = file_hashes
        .iter()
        .map(|file_hash| (file_hash.path.as_str(), file_hash))
        .collect::<BTreeMap<_, _>>();
    let mut import_files = Vec::with_capacity(request.files.len());
    for (file_index, file) in request.files.iter().enumerate() {
        ensure_source_index_cold_assembly_budget(
            cold_assembly_started,
            "file-read",
            file_index,
            request.files.len(),
        )?;
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
            selectors: file.selector_receipts.clone(),
        });
    }
    build_source_index_import_from_started(
        ClientDbSourceIndexImportRequest {
            generation_id: request.generation_id,
            project_root: request.project_root,
            schema_id: request.schema_id,
            schema_version: request.schema_version,
            selector_source: request.selector_source,
            file_hashes,
            files: import_files,
        },
        cold_assembly_started,
    )
}

/// Build the DB-owned source-index import packet from collected file facts.
pub fn build_source_index_import(
    request: ClientDbSourceIndexImportRequest,
) -> Result<ClientDbSourceIndexImport, String> {
    build_source_index_import_from_started(request, std::time::Instant::now())
}

const SOURCE_INDEX_COLD_ASSEMBLY_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);

fn build_source_index_import_from_started(
    request: ClientDbSourceIndexImportRequest,
    cold_assembly_started: std::time::Instant,
) -> Result<ClientDbSourceIndexImport, String> {
    let file_hash_by_path = request
        .file_hashes
        .iter()
        .map(|file_hash| (file_hash.path.as_str(), file_hash))
        .collect::<BTreeMap<_, _>>();
    let mut owners = Vec::with_capacity(request.files.len());
    let mut selectors = Vec::with_capacity(request.files.len());
    for (file_index, file) in request.files.iter().enumerate() {
        ensure_source_index_cold_assembly_budget(
            cold_assembly_started,
            "owner-selector-assembly",
            file_index,
            request.files.len(),
        )?;
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
            payload_proof: None,
        });
        for selector in &file.selectors {
            if selector.owner_path.as_str() != relative_path {
                return Err(format!(
                    "source index selector owner mismatch: file={} selectorOwner={} selector={}",
                    relative_path,
                    selector.owner_path.as_str(),
                    selector.selector_id
                ));
            }
            selectors.push(selector.clone());
        }
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

fn ensure_source_index_cold_assembly_budget(
    started: std::time::Instant,
    stage: &str,
    processed_files: usize,
    total_files: usize,
) -> Result<(), String> {
    let elapsed = started.elapsed();
    if elapsed < SOURCE_INDEX_COLD_ASSEMBLY_BUDGET {
        return Ok(());
    }

    Err(format!(
        "source-index cold assembly budget exhausted: stage={stage} budgetMs={} elapsedMs={} processedFiles={processed_files} totalFiles={total_files}",
        SOURCE_INDEX_COLD_ASSEMBLY_BUDGET.as_millis(),
        elapsed.as_millis(),
    ))
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

fn source_scope_evidence_hashes(registry_fingerprint: &str) -> Vec<ClientCacheFileHash> {
    vec![
        client_db_source_index_registry_evidence_hash(registry_fingerprint),
        ClientCacheFileHash {
            path: "@scope/source-index-layout/term-projection-v1".to_string(),
            sha256: format!(
                "{:x}",
                Sha256::digest(b"asp-source-index-term-projection-layout-v1")
            ),
            byte_len: 0,
            mtime_ms: 0,
        },
    ]
}

fn source_index_file_hash(
    project_root: &Path,
    file: &ClientDbSourceIndexScopeFile,
    previous_by_path: Option<&BTreeMap<&str, &ClientCacheFileHash>>,
    force_content_hash: bool,
) -> Result<ClientCacheFileHash, String> {
    let metadata = fs::metadata(&file.path).map_err(|error| {
        format!(
            "failed to read source index file metadata {}: {error}",
            file.path.display()
        )
    })?;
    let mtime_ms = metadata_mtime_ms(&metadata, &file.path)?;
    let relative_path = source_index_relative_path(project_root, &file.path);
    if !force_content_hash
        && let Some(previous) =
            previous_by_path.and_then(|hashes| hashes.get(relative_path.as_str()))
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

fn source_index_tracked_worktree_is_dirty(project_root: &Path) -> bool {
    let Some(git_root) = project_root
        .ancestors()
        .find(|candidate| candidate.join(".git").exists())
    else {
        return false;
    };
    let Ok(output) = Command::new("git")
        .arg("-C")
        .arg(git_root)
        .args(["diff", "--quiet", "HEAD", "--"])
        .output()
    else {
        return true;
    };
    !output.status.success()
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
