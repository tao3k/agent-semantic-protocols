//! DB-owned source-index import packet assembly.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use agent_semantic_client_core::ClientCacheFileHash;
use sha2::{Digest, Sha256};

use super::text::{source_line_count, source_query_keys};
use super::types::client_db_source_index_registry_evidence_hash;
use super::types::{
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
            relations: file.relations.clone(),
        });
    }
    build_source_index_import_inner(ClientDbSourceIndexImportRequest {
        generation_id: request.generation_id,
        project_root: request.project_root,
        schema_id: request.schema_id,
        schema_version: request.schema_version,
        selector_source: request.selector_source,
        file_hashes,
        source_blobs: request.source_blobs,
        files: import_files,
    })
}

/// Build the DB-owned source-index import packet from collected file facts.
pub fn build_source_index_import(
    request: ClientDbSourceIndexImportRequest,
) -> Result<ClientDbSourceIndexImport, String> {
    build_source_index_import_inner(request)
}

fn build_source_index_import_inner(
    request: ClientDbSourceIndexImportRequest,
) -> Result<ClientDbSourceIndexImport, String> {
    let mut canonical_file_hashes = request.file_hashes.clone();
    let mut file_hash_index_by_path = canonical_file_hashes.iter().enumerate().try_fold(
        BTreeMap::new(),
        |mut index_by_path, (index, file_hash)| {
            if index_by_path
                .insert(file_hash.path.clone(), index)
                .is_some()
            {
                return Err(format!(
                    "source index file hashes repeat owner path: {}",
                    file_hash.path
                ));
            }
            Ok(index_by_path)
        },
    )?;
    for (owner_path, source) in request.source_blobs.iter() {
        let sha256 = format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(source));
        if let Some(index) = file_hash_index_by_path.get(owner_path).copied() {
            let file_hash = &mut canonical_file_hashes[index];
            file_hash.sha256 = sha256;
            file_hash.byte_len = source.len() as u64;
        } else {
            let index = canonical_file_hashes.len();
            canonical_file_hashes.push(ClientCacheFileHash {
                path: owner_path.to_owned(),
                sha256,
                byte_len: source.len() as u64,
                mtime_ms: 0,
            });
            file_hash_index_by_path.insert(owner_path.to_owned(), index);
        }
    }
    for file in &request.files {
        let owner_path = ClientDbSourceIndexPath::from(file.relative_path.clone());
        let source = request.source_blobs.get(&owner_path).ok_or_else(|| {
            format!(
                "missing content-addressed source bytes for owner {}",
                owner_path.as_str()
            )
        })?;
        let file_hash_index = file_hash_index_by_path
            .get(file.relative_path.as_str())
            .ok_or_else(|| format!("missing source index hash for {}", file.relative_path))?;
        let file_hash = &canonical_file_hashes[*file_hash_index];
        debug_assert_eq!(file_hash.byte_len, source.len() as u64);
    }
    let mut owners = Vec::with_capacity(request.files.len());
    let mut selectors = Vec::with_capacity(request.files.len());
    let mut relations = Vec::new();
    for file in &request.files {
        let Some(file_hash_index) = file_hash_index_by_path.get(file.relative_path.as_str()) else {
            return Err(format!(
                "missing source index hash for {}",
                file.relative_path
            ));
        };
        let file_hash = &canonical_file_hashes[*file_hash_index];
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
        relations.extend(file.relations.iter().cloned().map(|relation| {
            crate::ClientDbSourceIndexOwnedRelation {
                owner_path: owner_path.clone(),
                relation,
            }
        }));
    }
    let selector_owner_paths = selectors
        .iter()
        .map(|selector| selector.owner_path.as_str())
        .collect::<BTreeSet<_>>();
    let selector_ids = selectors
        .iter()
        .map(|selector| selector.selector_id.as_str())
        .collect::<BTreeSet<_>>();
    relations.retain(|owned| {
        relation_endpoints_are_selector_bound(&owned.relation, &selector_owner_paths, &selector_ids)
    });
    relations.sort();
    relations.dedup();
    Ok(ClientDbSourceIndexImport {
        generation_id: request.generation_id,
        project_root: request.project_root,
        schema_id: request.schema_id,
        schema_version: request.schema_version,
        file_hashes: canonical_file_hashes,
        source_blobs: request.source_blobs,
        owners,
        selectors,
        relations,
    })
}

fn relation_endpoints_are_selector_bound(
    relation: &agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation,
    selector_owner_paths: &BTreeSet<&str>,
    selector_ids: &BTreeSet<&str>,
) -> bool {
    use agent_semantic_content_identity::provider_projection_relation::{
        PROVIDER_RELATION_ITEM_ENDPOINT_KIND, PROVIDER_RELATION_OWNER_ENDPOINT_KIND,
    };

    [&relation.from, &relation.to].into_iter().all(|endpoint| {
        if endpoint.kind == PROVIDER_RELATION_ITEM_ENDPOINT_KIND {
            return selector_ids.contains(endpoint.id.as_str());
        }
        if endpoint.kind == PROVIDER_RELATION_OWNER_ENDPOINT_KIND {
            let owner_path = endpoint.id.strip_prefix("owner:").unwrap_or(&endpoint.id);
            return selector_owner_paths.contains(owner_path);
        }
        false
    })
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
        push_component(
            &mut canonical,
            "selectorProvider",
            selector.provider_id.as_str(),
        );
        push_component(
            &mut canonical,
            "canonicalItemSelector",
            &serde_json::to_string(selector.projection_record.proof.canonical_item_selector())
                .expect("canonical item selector serialization is infallible"),
        );
        push_component(
            &mut canonical,
            "projectionRecord",
            &serde_json::to_string(&selector.projection_record)
                .expect("projection record serialization is infallible"),
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
