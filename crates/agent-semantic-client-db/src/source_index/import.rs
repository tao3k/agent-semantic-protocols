//! DB-owned source-index import packet assembly.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::text::{source_line_count, source_query_keys};
use super::types::{
    ClientDbSourceIndexImport, ClientDbSourceIndexImportRequest, ClientDbSourceIndexOwner,
    ClientDbSourceIndexPath, ClientDbSourceIndexQueryKey, ClientDbSourceIndexScopeFile,
    ClientDbSourceIndexSelector, ClientDbSourceIndexSource,
};

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
