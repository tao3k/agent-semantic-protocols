//! Import packet assembly for Rust SQL source-index rows.

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

pub(super) fn source_index_import(
    project_root: &Path,
    generation_id: CacheGenerationId,
    files: &[SourceIndexScopeFile],
) -> Result<ClientDbSourceIndexImport, String> {
    let mut file_hashes = Vec::with_capacity(files.len());
    let mut owners = Vec::with_capacity(files.len());
    let mut selectors = Vec::with_capacity(files.len());
    for file in files {
        let bytes = fs::read(&file.path).map_err(|error| {
            format!(
                "failed to read source index file {}: {error}",
                file.path.display()
            )
        })?;
        let relative_path = relative_project_path(project_root, &file.path);
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        file_hashes.push(ClientCacheFileHash {
            path: relative_path.clone(),
            sha256,
        });
        let text = if bytes.len() as u64 <= SOURCE_INDEX_FILE_BYTES_LIMIT {
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
            selector_id: format!("{relative_path}:1:{}", line_count.max(1)),
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

fn relative_project_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
