//! Dynamic owner-local item search for high-churn source files.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::dynamic_overlay::{
    DynamicOverlayDocument, DynamicOverlayNamespace, DynamicOverlayQuery,
    default_dynamic_overlay_search_backend,
};
use crate::dynamic_search::owner_item_parts::render::{display_path, render_code, render_frontier};
use crate::dynamic_search::owner_item_parts::search::OwnerItemMatch;

/// Language facade selected by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicSearchLanguage<'a>(&'a str);

impl<'a> DynamicSearchLanguage<'a> {
    /// Create a language facade token for dynamic search.
    #[must_use]
    pub fn new(value: &'a str) -> Self {
        Self(value)
    }

    fn as_str(self) -> &'a str {
        self.0
    }
}

/// Project roots used to resolve and render owner paths.
#[derive(Clone, Copy, Debug)]
pub struct DynamicSearchRoots<'a> {
    project_root: &'a Path,
    locator_root: &'a Path,
}

impl<'a> DynamicSearchRoots<'a> {
    /// Create the root pair used by dynamic owner search.
    #[must_use]
    pub fn new(project_root: &'a Path, locator_root: &'a Path) -> Self {
        Self {
            project_root,
            locator_root,
        }
    }
}

/// Owner file selected by the caller.
#[derive(Clone, Copy, Debug)]
pub struct DynamicOwnerPath<'a>(&'a Path);

impl<'a> DynamicOwnerPath<'a> {
    /// Create an owner path wrapper.
    #[must_use]
    pub fn new(value: &'a Path) -> Self {
        Self(value)
    }
}

/// Query text for owner-local item search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicOwnerQuery<'a>(&'a str);

impl<'a> DynamicOwnerQuery<'a> {
    /// Create an owner-local query wrapper.
    #[must_use]
    pub fn new(value: &'a str) -> Self {
        Self(value)
    }
}

/// Request for dynamic owner-local item search.
#[derive(Clone, Copy, Debug)]
pub struct DynamicOwnerItemsRequest<'a> {
    /// Language facade used for selector rendering.
    pub language: DynamicSearchLanguage<'a>,
    /// Project roots used to resolve the owner and render stable locators.
    pub roots: DynamicSearchRoots<'a>,
    /// Owner path to inspect from the current worktree.
    pub owner: DynamicOwnerPath<'a>,
    /// Query text to match against owner-local declarations.
    pub query: DynamicOwnerQuery<'a>,
    /// Owner items supplied by a language harness command/interface.
    pub items: &'a [DynamicOwnerItem],
}

/// Language-harness owner item projected into the dynamic search service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicOwnerItem {
    /// Display and selector term, normally the parser-owned declaration name.
    pub(crate) term: String,
    /// Provider-owned item kind.
    pub(crate) kind: String,
    /// Display start line supplied by the provider.
    pub(crate) start: usize,
    /// Display end line supplied by the provider.
    pub(crate) end: usize,
}

impl DynamicOwnerItem {
    pub fn new(term: impl Into<String>, kind: impl Into<String>, start: usize, end: usize) -> Self {
        Self {
            term: term.into(),
            kind: kind.into(),
            start,
            end,
        }
    }
}

/// Render an agent-facing owner-items frontier from the current worktree.
///
/// This is the dynamic search path: it does not write every dirty edit into the
/// durable DB. The caller still receives selector/source hints plus a compact
/// next command, while future Turso-backed overlay search can replace the local
/// extractor behind this request boundary.
#[must_use]
pub fn render_dynamic_owner_items_frontier(request: DynamicOwnerItemsRequest<'_>) -> String {
    let owner_path = resolved_owner_path(request.roots.project_root, request.owner.0);
    let display_owner = display_path(request.roots.locator_root, &owner_path);
    let matches =
        overlay_owner_item_matches(&display_owner, &owner_path, request.items, request.query.0);
    render_frontier(
        request.language.as_str(),
        &display_owner,
        request.query.0,
        &matches,
    )
}

/// Render bounded source snippets for dynamic owner-local item search.
///
/// This is the code projection for high-churn owner files. It uses the same
/// lightweight dynamic matcher as the frontier projection and avoids provider
/// startup or durable DB writes for every transient edit.
#[must_use]
pub fn render_dynamic_owner_items_code(request: DynamicOwnerItemsRequest<'_>) -> String {
    let owner_path = resolved_owner_path(request.roots.project_root, request.owner.0);
    let display_owner = display_path(request.roots.locator_root, &owner_path);
    let matches =
        overlay_owner_item_matches(&display_owner, &owner_path, request.items, request.query.0);
    render_code(
        request.language.as_str(),
        &display_owner,
        &owner_path,
        &matches,
    )
}

fn overlay_owner_item_matches(
    display_owner: &str,
    owner_path: &Path,
    items: &[DynamicOwnerItem],
    query: &str,
) -> Vec<OwnerItemMatch> {
    let namespace = DynamicOverlayNamespace::new(
        "dynamic-owner-items",
        "workspace",
        "worktree",
        "session",
        "dirty",
    );
    let documents = items
        .iter()
        .map(|item| OwnerItemInput {
            owner: display_owner.to_string(),
            item,
        })
        .map(|item| {
            DynamicOverlayDocument::owner_item(
                item.owner,
                item.item.kind.clone(),
                item.item.term.clone(),
                item.item.start,
                item.item.end,
                "dirty",
            )
        })
        .collect::<Vec<_>>();
    let mut overlay = default_dynamic_overlay_search_backend();
    overlay.upsert_documents(namespace.clone(), documents);
    let matches = overlay
        .search(
            &namespace,
            &DynamicOverlayQuery::new(query).owner_path(display_owner),
        )
        .into_iter()
        .filter_map(|hit| {
            let (start, end) = hit.document.display_range?;
            Some(OwnerItemMatch {
                start,
                end,
                kind: hit.document.kind,
                term: hit.document.name,
                rank: 0,
            })
        })
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return owner_local_source_matches(owner_path, items, query);
    }
    matches
}

fn owner_local_source_matches(
    owner_path: &Path,
    items: &[DynamicOwnerItem],
    query: &str,
) -> Vec<OwnerItemMatch> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let Ok(source) = fs::read_to_string(owner_path) else {
        return Vec::new();
    };
    let query = query.to_ascii_lowercase();
    let mut matches = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        if !line.to_ascii_lowercase().contains(&query) {
            continue;
        }
        let line_number = line_index + 1;
        for item in items {
            if line_number < item.start || line_number > item.end {
                continue;
            }
            if matches.iter().any(|existing: &OwnerItemMatch| {
                existing.start == item.start
                    && existing.end == item.end
                    && existing.kind == item.kind
                    && existing.term == item.term
            }) {
                continue;
            }
            matches.push(OwnerItemMatch {
                start: item.start,
                end: item.end,
                kind: item.kind.clone(),
                term: item.term.clone(),
                rank: 1,
            });
        }
    }
    matches
}

struct OwnerItemInput<'a> {
    owner: String,
    item: &'a DynamicOwnerItem,
}

fn resolved_owner_path(project_root: &Path, owner: &Path) -> PathBuf {
    if owner.is_absolute() {
        owner.to_path_buf()
    } else {
        project_root.join(owner)
    }
}
