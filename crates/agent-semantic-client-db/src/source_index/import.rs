//! DB-owned source-index import packet assembly.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

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
    ClientDbSourceIndexSource,
};

/// Build source-index file hashes and import rows from collected workspace
/// files. Raw source text is used only transiently for owner/query projection.
pub fn assemble_source_index_import(
    request: ClientDbSourceIndexImportAssemblyRequest,
) -> Result<ClientDbSourceIndexImport, String> {
    let file_hashes = source_index_file_hashes(
        &request.project_root,
        &request.files,
        &request.source_blobs,
        &request.registry_fingerprint,
        request.extra_scope_dirs.iter().map(String::as_str),
    )?;
    source_index_import_with_file_hashes(request, file_hashes)
}

/// Import parser-owned language projection rows without projecting raw source text.
pub fn source_index_import_from_language_projection(
    request: ClientDbLanguageProjectionImportRequest,
) -> Result<super::language_projection::ClientDbLanguageProjectionImport, String> {
    let rows = language_projection_source_index_rows(&request.projection, &request.project_root)?;
    let file_hashes = source_index_file_hashes(
        &request.project_root,
        &rows.scope_files,
        &request.source_blobs,
        &request.registry_fingerprint,
        std::iter::empty(),
    )?;
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        file_hashes
            .iter()
            .map(|file_hash| (file_hash.path.as_str(), file_hash.sha256.as_str())),
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        agent_semantic_content_identity::provider_digest(request.registry_fingerprint.as_bytes()),
    );
    let generation_id =
        crate::source_index::client_db_source_index_generation_id_for_snapshot(&source_snapshot);
    Ok(
        super::language_projection::ClientDbLanguageProjectionImport {
            source_index: ClientDbSourceIndexImport {
                generation_id,
                project_root: request.project_root,
                schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
                schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
                file_hashes,
                owners: rows.owners,
                selectors: rows.selectors,
            },
            source_snapshot,
            membership_change_set:
                super::types::ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
        },
    )
}

/// Return source-index file and scope evidence hashes without assembling rows.
pub fn source_index_file_hashes<'a>(
    project_root: &Path,
    files: &[ClientDbSourceIndexScopeFile],
    source_blobs: &super::types::ClientDbSourceIndexSourceBlobs,
    registry_fingerprint: &str,
    extra_scope_dirs: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<ClientCacheFileHash>, String> {
    let mut file_hashes = files
        .iter()
        .map(|file| source_index_file_hash(project_root, file, source_blobs))
        .collect::<Result<Vec<_>, _>>()?;
    file_hashes.extend(files.iter().map(source_index_selector_evidence_hash));
    let mut scope_dirs = extra_scope_dirs
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    scope_dirs.sort();
    scope_dirs.dedup();
    for relative_dir in scope_dirs {
        let scope_path = project_root.join(&relative_dir);
        let metadata = fs::metadata(&scope_path).map_err(|error| {
            format!(
                "failed to read source-index scope anchor {}: {error}",
                scope_path.display()
            )
        })?;
        let mtime_ms = metadata_mtime_ms(&metadata, &scope_path)?;
        file_hashes.push(
            super::types::client_db_source_index_scope_dir_evidence_hash(
                &relative_dir,
                metadata.len(),
                mtime_ms,
            ),
        );
    }
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
            let source_blob_path = ClientDbSourceIndexPath::new(relative_path.clone());
            let bytes = request
                .source_blobs
                .get(&source_blob_path)
                .ok_or_else(|| format!("missing same-pass source blob for {relative_path}"))?;
            String::from_utf8(bytes.to_vec()).unwrap_or_default()
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
        for selector in &file.selectors {
            if selector.owner_path.as_str() != relative_path {
                return Err(format!(
                    "source index selector owner mismatch: file={} selectorOwner={} selector={}",
                    relative_path,
                    selector.owner_path.as_str(),
                    selector.selector_id.as_str()
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

fn source_index_selector_evidence_hash(file: &ClientDbSourceIndexScopeFile) -> ClientCacheFileHash {
    fn push_component(canonical: &mut String, label: &str, value: &str) {
        use std::fmt::Write as _;
        let _ = writeln!(canonical, "{label}:{}:{value}", value.len());
    }

    fn digest_hex(digest: &[u8; 32]) -> String {
        use std::fmt::Write as _;
        digest.iter().fold(
            String::with_capacity(digest.len() * 2),
            |mut encoded, byte| {
                let _ = write!(encoded, "{byte:02x}");
                encoded
            },
        )
    }

    let mut selectors = file.selector_receipts.iter().collect::<Vec<_>>();
    selectors.sort_by(|left, right| {
        left.owner_path
            .as_str()
            .cmp(right.owner_path.as_str())
            .then_with(|| left.selector_id.as_str().cmp(right.selector_id.as_str()))
    });
    let mut canonical = String::from("asp.source-index.selector-generation.v1\n");
    push_component(&mut canonical, "path", file.path.to_string_lossy().as_ref());
    push_component(&mut canonical, "language", file.language_id.as_str());
    push_component(&mut canonical, "provider", file.provider_id.as_str());
    for selector in selectors {
        push_component(&mut canonical, "owner", selector.owner_path.as_str());
        push_component(&mut canonical, "selector", selector.selector_id.as_str());
        push_component(
            &mut canonical,
            "symbol",
            selector.symbol.as_ref().map_or("", |value| value.as_str()),
        );
        push_component(
            &mut canonical,
            "kind",
            selector.kind.as_ref().map_or("", |value| value.as_str()),
        );
        push_component(&mut canonical, "source", selector.source.as_str());
        for query_key in &selector.query_keys {
            push_component(&mut canonical, "queryKey", query_key.as_str());
        }
        let proof = &selector.materialization_proof;
        push_component(
            &mut canonical,
            "materializationSelector",
            &proof.structural_selector,
        );
        push_component(&mut canonical, "materializationOwner", &proof.owner_path);
        push_component(
            &mut canonical,
            "workspaceRootDigest",
            &digest_hex(&proof.workspace_root_digest),
        );
        push_component(
            &mut canonical,
            "ownerSubtreeDigest",
            &digest_hex(&proof.owner_subtree_digest),
        );
        push_component(
            &mut canonical,
            "sourceBlobDigest",
            &digest_hex(&proof.source_blob_digest),
        );
        push_component(
            &mut canonical,
            "parserFactDigest",
            &digest_hex(&proof.normalized_parser_facts_digest),
        );
        push_component(
            &mut canonical,
            "projectionDigest",
            &digest_hex(&proof.projection_digest),
        );
        push_component(
            &mut canonical,
            "sourceByteStart",
            &proof.source_byte_start.to_string(),
        );
        push_component(
            &mut canonical,
            "sourceByteEnd",
            &proof.source_byte_end.to_string(),
        );
    }
    ClientCacheFileHash {
        path: format!(
            "@scope/selector-generation/{}",
            file.path.to_string_lossy().replace('\\', "/")
        ),
        sha256: format!("{:x}", Sha256::digest(canonical.as_bytes())),
        byte_len: canonical.len() as u64,
        mtime_ms: 0,
    }
}

fn source_index_file_hash(
    project_root: &Path,
    file: &ClientDbSourceIndexScopeFile,
    source_blobs: &super::types::ClientDbSourceIndexSourceBlobs,
) -> Result<ClientCacheFileHash, String> {
    let source_path = if file.path.is_absolute() {
        file.path.clone()
    } else {
        project_root.join(&file.path)
    };
    let metadata = fs::metadata(&source_path).map_err(|error| {
        format!(
            "failed to read source index file metadata {}: {error}",
            source_path.display()
        )
    })?;
    let mtime_ms = metadata_mtime_ms(&metadata, &source_path)?;
    let relative_path = source_index_relative_path(project_root, &source_path);
    let bytes = source_blobs
        .get(&ClientDbSourceIndexPath::new(relative_path.clone()))
        .ok_or_else(|| format!("missing same-pass source blob for {relative_path}"))?;
    Ok(ClientCacheFileHash {
        path: relative_path,
        sha256: format!("{:x}", Sha256::digest(bytes)),
        byte_len: bytes.len() as u64,
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
