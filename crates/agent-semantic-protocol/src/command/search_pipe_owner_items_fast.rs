//! ASP-owned fast path for `search owner <path> items`.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use super::graph::GraphTurboReceiptRequest;
use super::language_owner_items::{
    LanguageOwnerItemsDispatchRequest, dispatch_language_owner_items,
};
use super::search_config::AspConfig;
use super::search_owner::{item::OwnerItem, rust_items::collect_syn_rust_owner_items};
use super::search_pipe_args::{OwnerQueryArgs, parse_search_owner_items_query_args};
use super::search_pipe_provider_facts::ProviderGraphFactsContext;
use super::search_pipe_view::reject_non_graph_turbo_receipt;
use agent_semantic_client::{
    LanguageOwnerItemsAttempt, LanguageOwnerItemsDispatchPlan, language_owner_items_workspace_root,
    run_language_owner_items_dispatch_plan,
};
use agent_semantic_runtime::language_owner_source_path;
use agent_semantic_search::{
    DynamicOwnerItem, DynamicOwnerItemsRequest, DynamicOwnerPath, DynamicOwnerQuery,
    DynamicSearchLanguage, DynamicSearchRoots, render_dynamic_owner_items_code,
    render_dynamic_owner_items_frontier,
};

pub(super) struct SearchOwnerItemsFastContext<'a> {
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) cache_home: &'a Path,
    pub(super) config: &'a AspConfig,
    pub(super) provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    pub(super) frontier_receipt: Option<&'a GraphTurboReceiptRequest>,
    pub(super) source_snapshot: &'a agent_semantic_content_identity::SourceSnapshotEvidence,
}

struct OwnerItemsSearchState<'a> {
    args: &'a [String],
    language_id: &'a str,
    owner_project_root: PathBuf,
    cache_home: &'a Path,
    config: &'a AspConfig,
    provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    owner: &'a Path,
    locator_root: &'a Path,
    source_snapshot: &'a agent_semantic_content_identity::SourceSnapshotEvidence,
}

impl<'a> OwnerItemsSearchState<'a> {
    fn new(
        args: &'a [String],
        context: SearchOwnerItemsFastContext<'a>,
        owner: &'a Path,
        owner_project_root: PathBuf,
    ) -> Self {
        Self {
            args,
            language_id: context.language_id,
            owner_project_root,
            cache_home: context.cache_home,
            config: context.config,
            provider_context: context.provider_context,
            owner,
            locator_root: context.locator_root,
            source_snapshot: context.source_snapshot,
        }
    }

    fn try_dynamic_owner_items(&self, owner_query_args: &OwnerQueryArgs) -> Result<bool, String> {
        if self.source_snapshot.root_digest.is_empty() {
            return Err("dynamic owner-items requires a pinned Merkle root".to_string());
        }
        if self.language_id == "org" && owner_query_args.view == "seeds" {
            let owner_path = language_owner_source_path(&self.owner_project_root, self.owner);
            let document_items = collect_org_document_owner_items(&owner_path)?;
            if !document_items.is_empty() {
                render_org_document_owner_items_frontier(
                    self.owner,
                    &owner_query_args.query,
                    &document_items,
                );
                return Ok(true);
            }
        }
        try_render_dynamic_owner_items(
            self.language_id,
            &self.owner_project_root,
            self.locator_root,
            self.owner,
            &owner_query_args.query,
            &owner_query_args.view,
        )
    }

    fn try_provider(&self) -> Result<LanguageOwnerItemsAttempt, String> {
        Ok(
            dispatch_language_owner_items(LanguageOwnerItemsDispatchRequest {
                language_id: self.language_id,
                args: self.args,
                owner: self.owner,
                project_root: &self.owner_project_root,
                cache_home: self.cache_home,
                config: self.config,
                provider_context: self.provider_context,
            })?
            .into(),
        )
    }
}

fn emit_source_index_trace(_state: &OwnerItemsSearchState<'_>) -> Result<(), String> {
    Ok(())
}

pub(super) fn run_pre_activation_dynamic_rust_owner_items_search(
    args: &[String],
    project_root: &Path,
    locator_root: &Path,
) -> Result<bool, String> {
    if !matches!(
        (
            args.first().map(String::as_str),
            args.get(1).map(String::as_str),
            args.get(3).map(String::as_str),
        ),
        (Some("search"), Some("owner"), Some("items"))
    ) {
        return Ok(false);
    }
    let owner_query_args = parse_search_owner_items_query_args(args)?;
    if !matches!(owner_query_args.view.as_str(), "seeds" | "hits") {
        return Ok(false);
    }
    let owner_project_root = language_owner_items_workspace_root(
        project_root,
        locator_root,
        search_owner_items_workspace(args).as_deref(),
    );
    try_render_dynamic_owner_items(
        "rust",
        &owner_project_root,
        locator_root,
        &owner_query_args.owner,
        &owner_query_args.query,
        &owner_query_args.view,
    )
}

pub(super) fn run_search_owner_items_query_command(
    args: &[String],
    context: SearchOwnerItemsFastContext<'_>,
) -> Result<(), String> {
    reject_non_graph_turbo_receipt(context.frontier_receipt)?;
    let owner_query_args = parse_search_owner_items_query_args(args)?;
    if !matches!(owner_query_args.view.as_str(), "seeds" | "hits") {
        return Err(
            "search owner items fast path supports --view seeds or --view hits".to_string(),
        );
    }
    let owner_project_root = language_owner_items_workspace_root(
        context.project_root,
        context.locator_root,
        search_owner_items_workspace(args).as_deref(),
    );
    let state =
        OwnerItemsSearchState::new(args, context, &owner_query_args.owner, owner_project_root);
    emit_source_index_trace(&state)?;
    if state.try_dynamic_owner_items(&owner_query_args)? {
        return Ok(());
    }
    run_language_owner_items_dispatch_plan(LanguageOwnerItemsDispatchPlan {
        language_id: state.language_id,
        owner: state.owner,
        project_root: &state.owner_project_root,
        provider: || state.try_provider(),
    })?;
    Ok(())
}

fn search_owner_items_workspace(args: &[String]) -> Option<PathBuf> {
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--workspace" {
            return args.get(index + 1).map(PathBuf::from);
        }
        index += 1;
    }
    None
}

type DynamicOwnerItemCollector = fn(&Path) -> Result<Vec<DynamicOwnerItem>, String>;

fn dynamic_owner_item_collector(language_id: &str) -> Option<DynamicOwnerItemCollector> {
    match language_id {
        "rust" => Some(collect_dynamic_rust_owner_items),
        "org" => Some(collect_dynamic_org_owner_items),
        _ => None,
    }
}

fn try_render_dynamic_owner_items(
    language_id: &str,
    project_root: &Path,
    locator_root: &Path,
    owner: &Path,
    query: &str,
    view: &str,
) -> Result<bool, String> {
    let Some(output) =
        render_dynamic_owner_items(language_id, project_root, locator_root, owner, query, view)?
    else {
        return Ok(false);
    };
    print!("{output}");
    Ok(true)
}

fn render_dynamic_owner_items(
    language_id: &str,
    project_root: &Path,
    locator_root: &Path,
    owner: &Path,
    query: &str,
    view: &str,
) -> Result<Option<String>, String> {
    let Some(collector) = dynamic_owner_item_collector(language_id) else {
        return Ok(None);
    };
    let owner_path = language_owner_source_path(project_root, owner);
    let dynamic_items = collector(&owner_path)?;
    if dynamic_items.is_empty() {
        return Ok(None);
    }
    let request = DynamicOwnerItemsRequest {
        language: DynamicSearchLanguage::new(language_id),
        roots: DynamicSearchRoots::new(project_root, locator_root),
        owner: DynamicOwnerPath::new(owner),
        query: DynamicOwnerQuery::new(query),
        items: &dynamic_items,
    };
    let output = if view == "hits" {
        render_dynamic_owner_items_code(request)
    } else {
        render_dynamic_owner_items_frontier(request)
    };
    Ok(Some(output))
}

fn collect_dynamic_rust_owner_items(owner_path: &Path) -> Result<Vec<DynamicOwnerItem>, String> {
    let source = match fs::read_to_string(owner_path) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "failed to read Rust owner {}: {error}",
                owner_path.display()
            ));
        }
    };
    Ok(collect_syn_rust_owner_items(&source, owner_path)?
        .iter()
        .map(dynamic_owner_item_from_query_owner_item)
        .collect())
}

fn collect_dynamic_org_owner_items(owner_path: &Path) -> Result<Vec<DynamicOwnerItem>, String> {
    let source = match fs::read_to_string(owner_path) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "failed to read Org owner {}: {error}",
                owner_path.display()
            ));
        }
    };
    let mut items = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        let stars = line
            .chars()
            .take_while(|character| *character == '*')
            .count();
        if stars == 0 {
            continue;
        }
        let Some(rest) = line.get(stars..) else {
            continue;
        };
        if !rest.starts_with(' ') {
            continue;
        }
        let title = rest.trim();
        if title.is_empty() {
            continue;
        }
        let line_number = line_index + 1;
        items.push(DynamicOwnerItem::new(
            title,
            "heading",
            line_number,
            line_number,
        ));
    }
    Ok(items)
}

struct OrgDocumentOwnerItem {
    kind: &'static str,
    title: String,
    start_line: usize,
    end_line: usize,
}

fn collect_org_document_owner_items(
    owner_path: &Path,
) -> Result<Vec<OrgDocumentOwnerItem>, String> {
    let source = match fs::read_to_string(owner_path) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "failed to read Org owner {}: {error}",
                owner_path.display()
            ));
        }
    };
    let mut items = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        let stars = line
            .chars()
            .take_while(|character| *character == '*')
            .count();
        if stars == 0 {
            continue;
        }
        let Some(rest) = line.get(stars..) else {
            continue;
        };
        if !rest.starts_with(' ') {
            continue;
        }
        let title = rest.trim();
        if title.is_empty() {
            continue;
        }
        let line_number = line_index + 1;
        items.push(OrgDocumentOwnerItem {
            kind: "heading",
            title: title.to_string(),
            start_line: line_number,
            end_line: line_number,
        });
    }
    Ok(items)
}

fn render_org_document_owner_items_frontier(
    owner: &Path,
    query: &str,
    items: &[OrgDocumentOwnerItem],
) {
    let owner = owner.display();
    println!(
        "[search-owner] lang=org q={query} owner={owner} selector=items alg=asp-dynamic-owner-items-v1"
    );
    for item in items.iter().take(80) {
        let slug = document_heading_slug(&item.title);
        println!(
            "|item kind={} selector=\"org://{}#item/heading/{}\" title=\"{}\" range=\"{}:{}\"",
            item.kind, owner, slug, item.title, item.start_line, item.end_line
        );
    }
}

fn document_heading_slug(title: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for character in title.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "heading".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
#[path = "../../tests/unit/search_pipe_owner_items_fast.rs"]
mod latency_tests;

fn dynamic_owner_item_from_query_owner_item(item: &OwnerItem) -> DynamicOwnerItem {
    DynamicOwnerItem::new(item.name(), item.kind(), item.start_line(), item.end_line())
}
