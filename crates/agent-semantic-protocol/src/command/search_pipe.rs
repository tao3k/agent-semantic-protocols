//! ASP-owned search pipeline wrapper.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::graph::GraphTurboReceiptRequest;
use super::search_config::AspConfig;
use super::search_pipe_args::{
    parse_ingest_args, parse_lexical_args, parse_owner_only_args, parse_owner_query_args,
    parse_search_pipe_args,
};
use super::search_pipe_candidates::{parse_ingest_candidates, read_piped_stdin};
use super::search_pipe_failure::run_search_failure_command;
use super::search_pipe_model::SearchPipeSourceTrace;
use super::search_pipe_owner_items_fast::{
    SearchOwnerItemsFastContext, run_search_owner_items_query_command,
};
use super::search_pipe_provider_facts::{ProviderGraphFactsContext, collect_provider_graph_facts};
use super::search_pipe_read_memory::read_loop_memory_selectors;
use super::search_pipe_render::{render_empty_ingest_diagnostic, render_owner_tests_frontier};
use super::search_pipe_selector_seed::{
    SelectorSeedSearchPipeRequest, print_selector_seeded_search_pipe,
};
use super::search_pipe_source::{
    CandidateAcquisition, CollectSearchPipeCandidatesRequest, SourceSpec,
    collect_search_pipe_candidates,
};
use super::search_pipe_surfaces::normalized_search_surfaces;
use super::search_pipe_view::{
    SearchPipeViewRequest, print_search_pipe_view, reject_non_graph_turbo_receipt,
};
use super::search_query_budget::{
    SearchQueryBudgetBlock, search_query_budget_block, specific_search_term,
};
use super::search_suggest::{
    is_search_suggest, is_unsupported_search_pipeline_command,
    reject_unsupported_search_pipeline_command, run_search_suggest_command,
};
use agent_semantic_search::search_command_preflight::{
    SearchCommandPreflightOutcome, preflight_search_command_args,
};

pub(super) struct FastSearchContext<'a> {
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) cache_home: &'a Path,
    pub(super) config: &'a AspConfig,
    pub(super) provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    pub(super) frontier_receipt: Option<&'a GraphTurboReceiptRequest>,
    pub(super) source_index_snapshot:
        Option<&'a agent_semantic_client::source_index::CurrentSourceIndexSnapshot>,
}

impl FastSearchContext<'_> {
    fn required_source_index_snapshot(
        &self,
    ) -> Result<&agent_semantic_client::source_index::CurrentSourceIndexSnapshot, String> {
        self.source_index_snapshot
            .ok_or_else(|| "search route requires an admitted source-index generation".to_owned())
    }
}

pub(super) fn fast_search_requires_source_index_snapshot(args: &[String]) -> bool {
    is_search_pipe(args) || is_search_ingest(args) || is_search_lexical(args)
}

pub(super) struct IncrementalOwnerSearchContext<'a> {
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    pub(super) frontier_receipt: Option<&'a GraphTurboReceiptRequest>,
}

pub(super) fn run_asp_incremental_owner_search_command(
    args: &[String],
    context: IncrementalOwnerSearchContext<'_>,
) -> Result<(), String> {
    if !is_search_owner_items_query(args) {
        return Err("incremental owner search requires `search owner <path> items`".to_string());
    }
    match preflight_search_command_args(&context.language_id.into(), args, context.project_root) {
        SearchCommandPreflightOutcome::Rejected(error) => return Err(error),
        SearchCommandPreflightOutcome::Passed | SearchCommandPreflightOutcome::NotApplicable => {}
    }
    run_search_owner_items_query_command(
        args,
        SearchOwnerItemsFastContext {
            language_id: context.language_id,
            project_root: context.project_root,
            locator_root: context.locator_root,
            provider_context: context.provider_context,
            frontier_receipt: context.frontier_receipt,
        },
    )
}

pub(super) fn is_asp_fast_search(args: &[String]) -> bool {
    is_search_pipe(args)
        || is_search_suggest(args)
        || is_unsupported_search_pipeline_command(args)
        || is_search_ingest(args)
        || is_search_lexical(args)
        || is_search_failure(args)
        || is_reasoning_owner_query(args)
        || is_reasoning_owner_tests(args)
        || is_search_owner_items_query(args)
}

pub(super) fn run_asp_fast_search_command(
    args: &[String],
    context: FastSearchContext<'_>,
) -> Result<(), String> {
    if context.frontier_receipt.is_some()
        && (is_search_suggest(args)
            || is_unsupported_search_pipeline_command(args)
            || is_search_failure(args))
    {
        return Err(
            "--frontier-receipt-out is supported only for graph-turbo frontier search commands"
                .to_string(),
        );
    }
    if is_search_pipe(args) {
        return run_search_pipe_command(args, &context);
    }
    if is_search_suggest(args) {
        return run_search_suggest_command(context.language_id, args);
    }
    if is_unsupported_search_pipeline_command(args) {
        return reject_unsupported_search_pipeline_command();
    }
    if is_search_ingest(args) {
        return run_search_ingest_command(args, &context);
    }
    if is_search_owner_items_query(args) {
        match preflight_search_command_args(&context.language_id.into(), args, context.project_root)
        {
            SearchCommandPreflightOutcome::Rejected(error) => return Err(error),
            SearchCommandPreflightOutcome::Passed
            | SearchCommandPreflightOutcome::NotApplicable => {}
        }
        return run_search_owner_items_query_command(
            args,
            SearchOwnerItemsFastContext {
                language_id: context.language_id,
                project_root: context.project_root,
                locator_root: context.locator_root,
                provider_context: context.provider_context,
                frontier_receipt: context.frontier_receipt,
            },
        );
    }
    if is_search_lexical(args) {
        return run_search_lexical_command(args, &context);
    }
    if is_search_failure(args) {
        return run_search_failure_command(
            context.language_id,
            args,
            context.project_root,
            context.locator_root,
            context.cache_home,
            context.config,
        );
    }
    if is_reasoning_owner_query(args) {
        return run_reasoning_owner_query_command(
            context.language_id,
            args,
            context.project_root,
            context.locator_root,
            context.frontier_receipt,
        );
    }
    if is_reasoning_owner_tests(args) {
        return run_reasoning_owner_tests_command(
            args,
            context.project_root,
            context.locator_root,
            context.frontier_receipt,
        );
    }
    Err("unsupported ASP fast search command".to_string())
}

fn is_search_pipe(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("pipe"))
}

fn is_search_ingest(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("ingest"))
        && has_supported_fast_search_view(args)
        && !args.iter().any(|arg| arg == "--json")
}

fn is_search_lexical(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("lexical"))
        && parse_lexical_args(args)
            .is_ok_and(|request| matches!(request.view.as_str(), "seeds" | "graph-turbo-request"))
}

pub(super) fn is_parser_owned_treesitter_query(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("query"))
        && args.iter().any(|arg| arg == "--treesitter-query")
}

fn is_search_failure(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("failure"))
        && explicit_view(args).is_some_and(|view| matches!(view, "seeds" | "graph-turbo-request"))
        && !args.iter().any(|arg| arg == "--json")
}

fn is_reasoning_owner_query(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("reasoning"))
        && matches!(args.get(2).map(String::as_str), Some("owner-query"))
        && has_explicit_seed_view(args)
        && !args.iter().any(|arg| arg == "--json")
}

fn is_reasoning_owner_tests(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("reasoning"))
        && matches!(args.get(2).map(String::as_str), Some("owner-tests"))
        && has_explicit_seed_view(args)
        && !args.iter().any(|arg| arg == "--json")
}

pub(super) fn is_search_owner_items_query(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("owner"))
        && matches!(args.get(3).map(String::as_str), Some("items"))
        && has_supported_owner_items_view(args)
        && !args.iter().any(|arg| arg == "--json")
}

fn has_supported_owner_items_view(args: &[String]) -> bool {
    explicit_view(args).is_none_or(|view| matches!(view, "seeds" | "hits"))
}

fn has_explicit_seed_view(args: &[String]) -> bool {
    explicit_view(args).is_some_and(|view| view == "seeds")
}

fn has_supported_fast_search_view(args: &[String]) -> bool {
    match explicit_view(args) {
        Some(view) => matches!(view, "seeds" | "graph-turbo-request"),
        None => true,
    }
}

fn explicit_view(args: &[String]) -> Option<&str> {
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--view" {
            return args.get(index + 1).map(String::as_str);
        }
        index += 1;
    }
    None
}

fn run_search_pipe_command(args: &[String], context: &FastSearchContext<'_>) -> Result<(), String> {
    let pipe_args = parse_search_pipe_args(args)?;
    let project_root = search_workspace_root(
        context.project_root,
        context.locator_root,
        pipe_args.workspace.as_deref(),
    );
    if let Some(selector) = pipe_args.selector.as_deref() {
        return print_selector_seeded_search_pipe(SelectorSeedSearchPipeRequest {
            language_id: context.language_id,
            project_root: &project_root,
            locator_root: context.locator_root,
            selector,
            query: &pipe_args.seed_query,
            workspace: pipe_args.workspace.as_deref(),
            scopes: &pipe_args.scopes,
            view: &pipe_args.view,
            frontier_receipt: context.frontier_receipt,
        });
    }
    let budget_scopes = search_pipe_budget_scopes(&pipe_args);
    if pipe_args.view != "graph-turbo-request"
        && let Some(block) = super::search_pipe_provider_facts::with_query_pack_descriptor(
            context.provider_context,
            |query_pack_descriptor| {
                search_query_budget_block(agent_semantic_search::SearchQueryBudgetRequest {
                    language_id: context.language_id,
                    query: &pipe_args.seed_query,
                    scopes: &budget_scopes,
                    explicit_filters: false,
                    query_pack_descriptor,
                })
            },
        )?
    {
        print_search_query_budget_block(
            "search-pipe",
            context.language_id,
            &pipe_args.seed_query,
            pipe_args.workspace.as_deref(),
            &pipe_args.scopes,
            &block,
        );
        return Ok(());
    }
    let current_snapshot = context.required_source_index_snapshot()?;
    let mut acquisition = collect_search_pipe_candidates(CollectSearchPipeCandidatesRequest {
        language_id: context.language_id,
        project_root: &project_root,
        current_snapshot,
        locator_root: context.locator_root,
        intent: &pipe_args.seed_query,
        scopes: &pipe_args.scopes,
        source: pipe_args.source,
        config: context.config,
        provider_context: context.provider_context,
        require_multi_clause: true,
    })?;
    let provider_facts_started_at = Instant::now();
    let provider_facts = collect_provider_graph_facts(
        context.language_id,
        &project_root,
        Some(&pipe_args.seed_query),
        &acquisition.candidates,
        context.provider_context,
    )?;
    let source_trace = source_trace_with_provider_facts(
        &acquisition.source_trace,
        provider_facts_started_at.elapsed(),
        &provider_facts,
    );
    let rendered_source = resolved_search_pipe_source(pipe_args.source, &acquisition);
    let surfaces = normalized_search_surfaces(&pipe_args.surfaces);
    let source_snapshot = acquisition.source_snapshot.as_ref().ok_or_else(|| {
        "state=cold-required reasonKind=active-workspace-generation-required field=sourceSnapshot"
            .to_owned()
    })?;
    let generation =
        agent_semantic_search::graph_generation_authority::AdmittedGraphGenerationV1::admit(
            source_snapshot,
            &current_snapshot.workspace_generation,
            &current_snapshot.workspace_generation,
        )
        .map_err(|error| format!("search graph generation admission failed: {error}"))?;
    print_search_pipe_view(SearchPipeViewRequest {
        language_id: context.language_id,
        project_root: &project_root,
        source_snapshot,
        generation: &generation,
        locator_root: context.locator_root,
        cache_home: context.cache_home,
        surface: "search-pipe",
        query: Some(&pipe_args.seed_query),
        candidates: &acquisition.candidates,
        pipes: &surfaces,
        source: &rendered_source,
        candidate_sources: &acquisition.candidate_sources,
        source_trace: &source_trace,
        scopes: &pipe_args.scopes,
        view: &pipe_args.view,
        include_pipe_plan: true,
        provider_facts: &provider_facts,
        provider_context: context.provider_context,
        read_memory_selectors: &read_loop_memory_selectors(
            context.cache_home,
            &project_root,
            context.locator_root,
            &pipe_args.scopes,
        ),
        frontier_receipt: context.frontier_receipt,
    })?;
    Ok(())
}

fn resolved_search_pipe_source(source: SourceSpec, acquisition: &CandidateAcquisition) -> String {
    acquisition
        .candidate_sources
        .iter()
        .find(|candidate_source| candidate_source.as_str() != "provider")
        .or_else(|| acquisition.candidate_sources.first())
        .cloned()
        .unwrap_or_else(|| source.as_str().to_string())
}

fn search_pipe_budget_scopes(pipe_args: &super::search_pipe_args::SearchPipeArgs) -> Vec<PathBuf> {
    if !pipe_args.scopes.is_empty() {
        return pipe_args.scopes.clone();
    }
    pipe_args.workspace.clone().into_iter().collect::<Vec<_>>()
}

fn print_search_query_budget_block(
    surface: &str,
    language_id: &str,
    query: &str,
    workspace: Option<&Path>,
    scopes: &[PathBuf],
    block: &SearchQueryBudgetBlock,
) {
    println!("[{surface}] lang={language_id} view=seeds source=blocked ranker=query-budget");
    println!("query={query}");
    println!("queryQuality=blocked");
    if let Some(workspace) = workspace {
        println!("workspace={}", workspace.display());
    }
    if !scopes.is_empty() {
        println!(
            "scopes={}",
            scopes
                .iter()
                .map(|scope| scope.display().to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    println!(
        "noOutput reason={} sourceTrace=queryBudget:blocked[blocked=true;genericTerms={};termCount={}]",
        block.reason,
        block.generic_terms.join("|"),
        block.term_count
    );
    println!(
        "nextCommand={}",
        search_budget_next_command(language_id, query, workspace, scopes)
    );
    println!(
        "refineHint=use typed path-or-symbol clauses first; example: asp {language_id} search lexical 'path-or-symbol|error-code' --workspace <scope> --view seeds"
    );
    println!("avoid=repeat-search-pipe,broad-lexical,raw-read,raw-rg,workspace-wide-rg");
}

fn search_budget_next_command(
    language_id: &str,
    query: &str,
    workspace: Option<&Path>,
    scopes: &[PathBuf],
) -> String {
    let terms = super::search_query_budget::search_query_terms(query)
        .into_iter()
        .filter(|term| specific_search_term(term))
        .take(4)
        .collect::<Vec<_>>();
    let query = if terms.is_empty() {
        "path-or-symbol|error-code".to_string()
    } else {
        terms.join("|")
    };
    let scope = scopes
        .first()
        .map(|scope| scope.display().to_string())
        .or_else(|| workspace.map(|workspace| workspace.display().to_string()))
        .unwrap_or_else(|| "<scope>".to_string());
    format!(
        "asp {language_id} search lexical '{}' --workspace {} --view seeds",
        query,
        shell_arg(&scope)
    )
}

fn shell_arg(value: &str) -> String {
    if value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '.' | '/' | '-' | '_')
    }) {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
#[path = "../../tests/unit/search_pipe_source_generation_route.rs"]
mod source_generation_route_tests;

fn source_trace_with_provider_facts(
    source_trace: &[SearchPipeSourceTrace],
    elapsed: Duration,
    provider_facts: &super::search_pipe_provider_facts::ProviderGraphFacts,
) -> Vec<SearchPipeSourceTrace> {
    let mut trace = source_trace.to_vec();
    let node_count = provider_facts.nodes.len();
    let mut fields = BTreeMap::new();
    fields.insert(
        "elapsedMs".to_string(),
        Value::from(elapsed_millis(elapsed)),
    );
    fields.insert("nodes".to_string(), Value::from(node_count));
    fields.insert("edges".to_string(), Value::from(provider_facts.edges.len()));
    fields.insert(
        "inputCandidates".to_string(),
        Value::from(provider_facts.input_candidates),
    );
    fields.insert(
        "factCandidates".to_string(),
        Value::from(provider_facts.fact_candidates),
    );
    fields.insert(
        "truncatedCandidates".to_string(),
        Value::from(provider_facts.truncated_candidates),
    );
    if let Some(descriptor_id) = provider_facts.descriptor_id.as_ref() {
        fields.insert(
            "descriptorId".to_string(),
            Value::from(descriptor_id.clone()),
        );
    }
    if let Some(descriptor_version) = provider_facts.descriptor_version.as_ref() {
        fields.insert(
            "descriptorVersion".to_string(),
            Value::from(descriptor_version.clone()),
        );
    }
    fields.insert(
        "matchedAxes".to_string(),
        Value::Array(
            provider_facts
                .matched_axes
                .iter()
                .cloned()
                .map(Value::from)
                .collect(),
        ),
    );
    fields.insert(
        "matchedTerms".to_string(),
        Value::Array(
            provider_facts
                .matched_terms
                .iter()
                .cloned()
                .map(Value::from)
                .collect(),
        ),
    );
    let skipped = node_count == 0
        && provider_facts.edges.is_empty()
        && provider_facts.input_candidates == 0
        && provider_facts.fact_candidates == 0
        && provider_facts.truncated_candidates == 0
        && provider_facts.descriptor_id.is_none();
    let state = if skipped { "skipped" } else { "used" };
    trace.push(
        SearchPipeSourceTrace::new(
            "providerFacts",
            state,
            node_count,
            usize::from(skipped),
            node_count,
        )
        .with_fields(fields),
    );
    trace
}

fn elapsed_millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

pub(super) fn search_workspace_root(
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

pub(super) fn normalize_path(path: &Path) -> PathBuf {
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

fn run_reasoning_owner_query_command(
    language_id: &str,
    args: &[String],
    _project_root: &Path,
    _locator_root: &Path,
    frontier_receipt: Option<&GraphTurboReceiptRequest>,
) -> Result<(), String> {
    reject_non_graph_turbo_receipt(frontier_receipt)?;
    let owner_query_args = parse_owner_query_args(args)?;
    if owner_query_args.view != "seeds" {
        return Err("search reasoning owner-query fast path supports --view seeds".to_string());
    }
    Err(format!(
        "search reasoning owner-query for language `{language_id}` requires a language-harness owner-query interface for `{}`; ASP will not synthesize language items from source text",
        owner_query_args.owner.display()
    ))
}

fn run_reasoning_owner_tests_command(
    args: &[String],
    project_root: &Path,
    locator_root: &Path,
    frontier_receipt: Option<&GraphTurboReceiptRequest>,
) -> Result<(), String> {
    reject_non_graph_turbo_receipt(frontier_receipt)?;
    let owner_args = parse_owner_only_args(args, "owner-tests")?;
    if owner_args.view != "seeds" {
        return Err("search reasoning owner-tests fast path supports --view seeds".to_string());
    }
    print!(
        "{}",
        render_owner_tests_frontier(project_root, locator_root, &owner_args.owner)
    );
    Ok(())
}

fn run_search_ingest_command(
    args: &[String],
    context: &FastSearchContext<'_>,
) -> Result<(), String> {
    let ingest_args = parse_ingest_args(args)?;
    if !matches!(ingest_args.view.as_str(), "seeds" | "graph-turbo-request") {
        return Err(
            "search ingest fast path supports --view seeds or --view graph-turbo-request"
                .to_string(),
        );
    }
    let stdin = read_piped_stdin()?;
    if stdin
        .iter()
        .all(|byte| byte.is_ascii_whitespace() || *byte == 0)
    {
        print!("{}", render_empty_ingest_diagnostic(context.language_id));
        return Ok(());
    }
    let candidates =
        parse_ingest_candidates(context.project_root, context.locator_root, stdin.as_slice());
    let current_snapshot =
        agent_semantic_client::source_index::current_source_index_snapshot(context.project_root)?;
    let provider_facts = collect_provider_graph_facts(
        context.language_id,
        context.project_root,
        None,
        &candidates,
        context.provider_context,
    )?;
    let generation =
        agent_semantic_search::graph_generation_authority::AdmittedGraphGenerationV1::admit(
            &current_snapshot.source_snapshot,
            &current_snapshot.workspace_generation,
            &current_snapshot.workspace_generation,
        )
        .map_err(|error| format!("search graph generation admission failed: {error}"))?;
    print_search_pipe_view(SearchPipeViewRequest {
        language_id: context.language_id,
        project_root: context.project_root,
        source_snapshot: &current_snapshot.source_snapshot,
        generation: &generation,
        locator_root: context.locator_root,
        cache_home: context.cache_home,
        surface: "search-ingest",
        query: None,
        candidates: &candidates,
        pipes: &ingest_args.pipes,
        source: "ingest",
        candidate_sources: &["ingest".to_string()],
        source_trace: &[SearchPipeSourceTrace::new(
            "ingest",
            "used",
            candidates.len(),
            usize::from(candidates.is_empty()),
            candidates.len(),
        )],
        scopes: &[],
        view: &ingest_args.view,
        include_pipe_plan: false,
        provider_facts: &provider_facts,
        provider_context: context.provider_context,
        read_memory_selectors: &[],
        frontier_receipt: context.frontier_receipt,
    })?;
    Ok(())
}

fn run_search_lexical_command(
    args: &[String],
    context: &FastSearchContext<'_>,
) -> Result<(), String> {
    let pipe_args = parse_lexical_args(args)?;
    if !matches!(pipe_args.view.as_str(), "seeds" | "graph-turbo-request") {
        return Err(
            "search lexical supports --view seeds; GraphRouter is selected by the built-in route wrapper".to_string(),
        );
    }
    if let Some(block) = super::search_pipe_provider_facts::with_query_pack_descriptor(
        context.provider_context,
        |query_pack_descriptor| {
            search_query_budget_block(agent_semantic_search::SearchQueryBudgetRequest {
                language_id: context.language_id,
                query: &pipe_args.query,
                scopes: &pipe_args.owners,
                explicit_filters: false,
                query_pack_descriptor,
            })
        },
    )? {
        print_search_query_budget_block(
            "search-lexical",
            context.language_id,
            &pipe_args.query,
            None,
            &pipe_args.owners,
            &block,
        );
        return Ok(());
    }
    let project_root = search_workspace_root(
        context.project_root,
        context.locator_root,
        pipe_args.workspace.as_deref(),
    );
    if project_root != context.project_root {
        return Err(format!(
            "search lexical workspace identity mismatch: context={} requested={}",
            context.project_root.display(),
            project_root.display()
        ));
    }
    let current_snapshot = context.source_index_snapshot.ok_or_else(|| {
        "search lexical requires the provider-dispatch source-index snapshot".to_string()
    })?;
    let acquisition = collect_search_pipe_candidates(CollectSearchPipeCandidatesRequest {
        language_id: context.language_id,
        project_root: &project_root,
        current_snapshot,
        locator_root: context.locator_root,
        intent: &pipe_args.query,
        scopes: &pipe_args.owners,
        source: SourceSpec::Auto,
        config: context.config,
        provider_context: context.provider_context,
        require_multi_clause: false,
    })?;
    let provider_facts = collect_provider_graph_facts(
        context.language_id,
        &project_root,
        Some(&pipe_args.query),
        &acquisition.candidates,
        context.provider_context,
    )?;
    let source_label = acquisition
        .candidate_sources
        .first()
        .map(String::as_str)
        .unwrap_or("auto");
    let generation =
        agent_semantic_search::graph_generation_authority::AdmittedGraphGenerationV1::admit(
            &current_snapshot.source_snapshot,
            &current_snapshot.workspace_generation,
            &current_snapshot.workspace_generation,
        )
        .map_err(|error| format!("search graph generation admission failed: {error}"))?;
    print_search_pipe_view(SearchPipeViewRequest {
        language_id: context.language_id,
        project_root: &project_root,
        source_snapshot: &current_snapshot.source_snapshot,
        generation: &generation,
        locator_root: context.locator_root,
        cache_home: context.cache_home,
        surface: "search-lexical",
        query: Some(&pipe_args.query),
        candidates: &acquisition.candidates,
        pipes: &pipe_args.pipes,
        source: source_label,
        candidate_sources: &acquisition.candidate_sources,
        source_trace: &acquisition.source_trace,
        scopes: &pipe_args.owners,
        view: &pipe_args.view,
        include_pipe_plan: false,
        provider_facts: &provider_facts,
        provider_context: context.provider_context,
        read_memory_selectors: &[],
        frontier_receipt: context.frontier_receipt,
    })?;
    Ok(())
}
