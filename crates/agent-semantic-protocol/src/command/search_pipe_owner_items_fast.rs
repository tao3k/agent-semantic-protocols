//! ASP-owned fast path for `search owner <path> items`.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::graph::GraphTurboReceiptRequest;
use super::language_owner_items::{
    LanguageOwnerItemsDispatchRequest, dispatch_language_owner_items,
};
use super::search_config::AspConfig;
use super::search_pipe_args::parse_search_owner_items_query_args;
use super::search_pipe_provider_facts::ProviderGraphFactsContext;
use super::search_pipe_view::reject_non_graph_turbo_receipt;
use agent_semantic_client::{
    LanguageOwnerItemsAttempt, LanguageOwnerItemsDispatchPlan, language_owner_items_workspace_root,
    run_language_owner_items_dispatch_plan,
};
use agent_semantic_search::{
    OwnerItemsSourceIndexTraceRender, OwnerItemsSourceIndexTraceStream,
    owner_items_source_index_trace,
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
        let owner_project_root = language_owner_items_workspace_root(
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

fn emit_source_index_trace(state: &OwnerItemsSearchState<'_>) -> Result<(), String> {
    if let Some(trace) = owner_items_source_index_trace(&state.owner_project_root, state.owner)? {
        let mut stdout = io::stdout().lock();
        let mut stderr = io::stderr().lock();
        let mut trace = trace.render();
        append_source_index_search_frame_receipt(
            &mut trace.line,
            state.args,
            state.language_id,
            &state.owner_project_root,
            state.owner,
        );
        write_source_index_trace(trace, &mut stdout, &mut stderr)?;
    }
    Ok(())
}

fn append_source_index_search_frame_receipt(
    line: &mut String,
    args: &[String],
    language_id: &str,
    owner_project_root: &Path,
    owner: &Path,
) {
    let owner = owner
        .strip_prefix(owner_project_root)
        .unwrap_or(owner)
        .to_string_lossy()
        .replace('\\', "/");
    let workspace = search_owner_items_workspace(args)
        .map(|workspace| workspace.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| ".".to_string());
    let item_query = search_owner_items_query(args).unwrap_or("*");
    let view = search_owner_items_view(args).unwrap_or("seeds");
    let rerun_command = format!(
        "asp {} search owner {} items --query {} --workspace {} --view {}",
        language_id,
        shell_arg(&owner),
        shell_arg(item_query),
        shell_arg(&workspace),
        shell_arg(view)
    );
    let source_trace = format!("source-index:{owner}");
    let where_frame = format!("owner:{owner}");
    let (recommended_next, action_frontier, how_frame, next_command) =
        if line.contains("status=busy") {
            (
                "retry-source-index-lookup",
                "retry-source-index-lookup,owner-items-dispatch",
                "bounded-retry",
                rerun_command,
            )
        } else if line.contains("status=missing-db") || line.contains("status=empty-index") {
            (
                "refresh-source-index",
                "refresh-source-index,owner-items-dispatch",
                "refresh-cache-then-search",
                "asp cache source-index refresh --workspace .".to_string(),
            )
        } else if line.contains("status=hit") {
            (
                "search-owner-items",
                "search-owner-items,query-exact-selector",
                "owner-items-search",
                rerun_command,
            )
        } else {
            (
                "search-owner-items",
                "search-owner-items,revise-owner",
                "owner-items-search",
                rerun_command,
            )
        };
    line.push_str(&format!(
        " nextCommand={} recommendedNext={recommended_next} actionFrontier={action_frontier} sourceTrace={} avoid=inline-code-in-search,raw-read,repeat-owner whereFrame={} howFrame={how_frame}",
        quote_search_frame_value(&next_command),
        quote_search_frame_value(&source_trace),
        quote_search_frame_value(&where_frame)
    ));
}

fn search_owner_items_query(args: &[String]) -> Option<&str> {
    flag_value(args, "--query").or_else(|| flag_value(args, "-q"))
}

fn search_owner_items_view(args: &[String]) -> Option<&str> {
    flag_value(args, "--view")
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let mut index = 0;
    while index < args.len() {
        if args[index] == flag {
            return args.get(index + 1).map(String::as_str);
        }
        index += 1;
    }
    None
}

fn shell_arg(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn quote_search_frame_value(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn write_source_index_trace(
    trace: OwnerItemsSourceIndexTraceRender,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<LanguageOwnerItemsAttempt, String> {
    match trace.stream {
        OwnerItemsSourceIndexTraceStream::Stdout => writeln!(stdout, "{}", trace.line)
            .map_err(|error| format!("failed to write source-index hit: {error}"))?,
        OwnerItemsSourceIndexTraceStream::Stderr => writeln!(stderr, "{}", trace.line)
            .map_err(|error| format!("failed to write source-index trace: {error}"))?,
    }
    Ok(LanguageOwnerItemsAttempt::Unsupported)
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
    emit_source_index_trace(&state)?;
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
