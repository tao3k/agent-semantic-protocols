//! ASP-owned fast path for `search owner <path> items`.

use std::path::{Component, Path, PathBuf};

use super::graph::GraphTurboReceiptRequest;
use super::language_owner_items::{
    LanguageOwnerItemsDispatchRequest, dispatch_language_owner_items,
};
use super::search_config::AspConfig;
use super::search_pipe_args::parse_search_owner_items_query_args;
use super::search_pipe_provider_facts::ProviderGraphFactsContext;
use super::search_pipe_view::reject_non_graph_turbo_receipt;
use agent_semantic_client::{
    LanguageOwnerItemsAttempt, LanguageOwnerItemsDispatchPlan,
    run_language_owner_items_dispatch_plan,
};

pub(super) struct SearchOwnerItemsFastContext<'a> {
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) cache_home: &'a Path,
    pub(super) config: &'a AspConfig,
    pub(super) provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    pub(super) frontier_receipt: Option<&'a GraphTurboReceiptRequest>,
}

struct OwnerItemsSearchState<'a> {
    args: &'a [String],
    language_id: &'a str,
    owner_project_root: PathBuf,
    cache_home: &'a Path,
    config: &'a AspConfig,
    provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    owner: &'a Path,
}

impl<'a> OwnerItemsSearchState<'a> {
    fn new(args: &'a [String], context: SearchOwnerItemsFastContext<'a>, owner: &'a Path) -> Self {
        let owner_project_root = search_workspace_root(
            context.project_root,
            context.locator_root,
            search_owner_items_workspace(args).as_deref(),
        );
        Self {
            args,
            language_id: context.language_id,
            owner_project_root,
            cache_home: context.cache_home,
            config: context.config,
            provider_context: context.provider_context,
            owner,
        }
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
    let state = OwnerItemsSearchState::new(args, context, &owner_query_args.owner);
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

fn search_workspace_root(
    project_root: &Path,
    locator_root: &Path,
    explicit_workspace: Option<&Path>,
) -> PathBuf {
    let Some(workspace) = explicit_workspace else {
        return project_root.to_path_buf();
    };
    let workspace = if workspace.is_absolute() {
        workspace.to_path_buf()
    } else {
        locator_root.join(workspace)
    };
    normalize_path(&workspace)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(value) => normalized.push(value),
            Component::RootDir | Component::Prefix(_) => normalized.push(component.as_os_str()),
        }
    }
    normalized
}
