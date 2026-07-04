use std::fs;
use std::path::Path;

use super::item::owner_item_matches_request;
use super::owner_path::{owner_path_is_file_like, resolve_owner_path};
use super::python_imports::python_imported_owner_items;
use super::render::{
    render_code_matches, render_full_source, render_locator_matches, render_non_source_owner_query,
    render_unresolved_owner_query,
};
use super::request::OwnerQueryRequest;
use super::rust_items::collect_syn_rust_owner_items;
use super::tree_sitter_items::collect_tree_sitter_owner_items;

pub(in crate::command) fn run_asp_fast_owner_query_command(
    language_id: &str,
    args: &[String],
    project_root: &Path,
    locator_root: &Path,
) -> Result<bool, String> {
    let Some(request) = OwnerQueryRequest::parse(language_id, args) else {
        return Ok(false);
    };
    let Some(path) = resolve_owner_path(project_root, locator_root, &request.owner_path) else {
        if owner_path_is_file_like(&request.owner_path) {
            render_unresolved_owner_query(&request)?;
            return Ok(true);
        }
        return Ok(false);
    };
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let items = if language_id == "rust" {
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            render_non_source_owner_query(&request, &path, project_root, locator_root, &source)?;
            return Ok(true);
        }
        collect_syn_rust_owner_items(&source, &path)?
    } else {
        let Some(items) = collect_tree_sitter_owner_items(language_id, &source, &path)? else {
            if language_id == "python" && request.code {
                if let Some(imported) = python_imported_owner_items(
                    project_root,
                    locator_root,
                    &path,
                    &source,
                    &request.term,
                )? {
                    render_full_source(&imported.source)?;
                    return Ok(true);
                }
            }
            render_non_source_owner_query(&request, &path, project_root, locator_root, &source)?;
            return Ok(true);
        };
        items
    };
    let matches = items
        .iter()
        .filter(|item| {
            owner_item_matches_request(
                item,
                &request.language_id,
                &request.term,
                request.kind.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    if language_id == "python" && matches.is_empty() {
        return run_python_import_fallback(&request, project_root, locator_root, &path, &source);
    }

    if request.code {
        render_code_matches(&source, &matches)?;
    } else {
        render_locator_matches(
            &request,
            &path,
            project_root,
            locator_root,
            source.lines().count(),
            &matches,
        )?;
    }
    Ok(true)
}

fn run_python_import_fallback(
    request: &OwnerQueryRequest,
    project_root: &Path,
    locator_root: &Path,
    path: &Path,
    source: &str,
) -> Result<bool, String> {
    if let Some(imported) =
        python_imported_owner_items(project_root, locator_root, path, source, &request.term)?
    {
        let imported_matches = imported
            .items
            .iter()
            .filter(|item| {
                owner_item_matches_request(
                    item,
                    &request.language_id,
                    &request.term,
                    request.kind.as_deref(),
                )
            })
            .collect::<Vec<_>>();
        if request.code {
            if imported_matches.is_empty() {
                render_full_source(&imported.source)?;
            } else {
                render_code_matches(&imported.source, &imported_matches)?;
            }
            return Ok(true);
        }
        if !imported_matches.is_empty() {
            render_locator_matches(
                request,
                &imported.path,
                project_root,
                locator_root,
                imported.source.lines().count(),
                &imported_matches,
            )?;
            return Ok(true);
        }
    }
    if request.code {
        render_code_matches(source, &[])?;
        return Ok(true);
    }
    Ok(false)
}
