use std::fs;
use std::path::Path;

use super::item::{OwnerItem, owner_item_kind_matches_request, owner_item_matches_request};
use super::owner_path::{owner_path_is_file_like, resolve_owner_path};
use super::python_imports::python_imported_owner_items;
use super::render::{
    format_code_matches, format_full_source, format_locator_matches, format_non_source_owner_query,
    format_unresolved_owner_query, render_empty_code_match_error, write_owner_query_stdout,
};
use super::request::OwnerQueryRequest;
use super::rust_items::collect_syn_rust_owner_items;
use super::tree_sitter_items::collect_tree_sitter_owner_items;

pub(crate) fn run_asp_fast_owner_query_to_string(
    language_id: &str,
    args: &[String],
    project_root: &Path,
    locator_root: &Path,
) -> Result<Option<String>, String> {
    let Some(request) = OwnerQueryRequest::parse(language_id, args)? else {
        return Ok(None);
    };
    let Some(path) = resolve_owner_path(project_root, locator_root, &request.owner_path) else {
        if owner_path_is_file_like(&request.owner_path) {
            return Ok(Some(format_unresolved_owner_query(&request)?));
        }
        return Ok(None);
    };
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let Some(item_query) = request.item_query() else {
        return Err("owner query internal error: missing item query projection".to_string());
    };
    let items = if language_id == "rust" {
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            return Ok(Some(format_non_source_owner_query(
                &request,
                item_query,
                &path,
                project_root,
                locator_root,
                &source,
            )?));
        }
        collect_syn_rust_owner_items(&source, &path)?
    } else {
        let Some(items) = collect_tree_sitter_owner_items(language_id, &source, &path)? else {
            if language_id == "python"
                && item_query.is_code_projection()
                && let Some(imported) = python_imported_owner_items(
                    project_root,
                    locator_root,
                    &path,
                    &source,
                    item_query.term(),
                )?
            {
                return Ok(Some(format_full_source(&imported.source)));
            }
            return Ok(Some(format_non_source_owner_query(
                &request,
                item_query,
                &path,
                project_root,
                locator_root,
                &source,
            )?));
        };
        items
    };
    let mut matches = items
        .iter()
        .filter(|item| {
            owner_item_matches_request(
                item,
                &request.language_id,
                item_query.term(),
                item_query.kind(),
            )
        })
        .collect::<Vec<_>>();
    if matches.is_empty() {
        matches = owner_local_source_matches(
            &items,
            &source,
            item_query.term(),
            &request.language_id,
            item_query.kind(),
        );
    }
    let same_name_kinds = if matches.is_empty() {
        items
            .iter()
            .filter(|item| item.name() == item_query.term())
            .map(|item| item.kind())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if language_id == "python" && matches.is_empty() {
        return run_python_import_fallback_to_string(
            &request,
            item_query,
            project_root,
            locator_root,
            &path,
            &source,
            &same_name_kinds,
        );
    }

    if item_query.is_code_projection() {
        if matches.is_empty() {
            render_empty_code_match_error(
                &request,
                item_query,
                &path,
                project_root,
                locator_root,
                &same_name_kinds,
            )?;
            unreachable!("render_empty_code_match_error always returns Err for code misses");
        } else {
            Ok(Some(format_code_matches(&source, &matches)))
        }
    } else {
        Ok(Some(format_locator_matches(
            &request,
            item_query,
            &path,
            project_root,
            locator_root,
            source.lines().count(),
            &matches,
        )))
    }
}

fn owner_local_source_matches<'a>(
    items: &'a [OwnerItem],
    source: &str,
    term: &str,
    language_id: &str,
    selector_kind: Option<&str>,
) -> Vec<&'a OwnerItem> {
    let term = term.trim();
    if term.is_empty() {
        return Vec::new();
    }
    let term = term.to_ascii_lowercase();
    let mut matches: Vec<&OwnerItem> = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        if !line.to_ascii_lowercase().contains(&term) {
            continue;
        }
        let line_number = line_index + 1;
        for item in items {
            if line_number < item.start_line()
                || line_number > item.end_line()
                || !owner_item_kind_matches_request(item, language_id, selector_kind)
            {
                continue;
            }
            if !matches.iter().any(|existing| {
                existing.name() == item.name()
                    && existing.kind() == item.kind()
                    && existing.start_line() == item.start_line()
                    && existing.end_line() == item.end_line()
            }) {
                matches.push(item);
            }
        }
    }
    matches
}

pub(in crate::command) fn run_asp_fast_owner_query_command(
    language_id: &str,
    args: &[String],
    project_root: &Path,
    locator_root: &Path,
) -> Result<bool, String> {
    let Some(rendered) =
        run_asp_fast_owner_query_to_string(language_id, args, project_root, locator_root)?
    else {
        return Ok(false);
    };
    write_owner_query_stdout(&rendered)?;
    Ok(true)
}

fn run_python_import_fallback_to_string(
    request: &OwnerQueryRequest,
    item_query: &super::request::OwnerItemQuery,
    project_root: &Path,
    locator_root: &Path,
    path: &Path,
    source: &str,
    same_name_kinds: &[&str],
) -> Result<Option<String>, String> {
    if let Some(imported) =
        python_imported_owner_items(project_root, locator_root, path, source, item_query.term())?
    {
        let imported_matches = imported
            .items
            .iter()
            .filter(|item| {
                owner_item_matches_request(
                    item,
                    &request.language_id,
                    item_query.term(),
                    item_query.kind(),
                )
            })
            .collect::<Vec<_>>();
        if item_query.is_code_projection() {
            if imported_matches.is_empty() {
                return Ok(Some(format_full_source(&imported.source)));
            } else {
                return Ok(Some(format_code_matches(
                    &imported.source,
                    &imported_matches,
                )));
            }
        }
        if !imported_matches.is_empty() {
            return Ok(Some(format_locator_matches(
                request,
                item_query,
                &imported.path,
                project_root,
                locator_root,
                imported.source.lines().count(),
                &imported_matches,
            )));
        }
    }
    if item_query.is_code_projection() {
        render_empty_code_match_error(
            request,
            item_query,
            path,
            project_root,
            locator_root,
            same_name_kinds,
        )?;
        unreachable!("render_empty_code_match_error always returns Err for code misses");
    }
    Ok(None)
}
