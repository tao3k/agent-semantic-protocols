//! ASP-owned search pipeline wrapper.

use std::fs;
use std::path::Path;

use super::graph::GraphTurboReceiptRequest;
use super::search_config::AspConfig;
use super::search_failure_render::{render_failure_frontier, render_failure_graph_turbo_request};
use super::search_pipe_args::{
    parse_failure_args, parse_fzf_args, parse_ingest_args, parse_owner_only_args,
    parse_owner_query_args, parse_search_owner_items_query_args, parse_search_pipe_args,
};
use super::search_pipe_candidates::{
    collect_candidates, parse_ingest_candidates, read_piped_stdin,
};
use super::search_pipe_model::SearchPipeSourceTrace;
use super::search_pipe_owner_query::render_owner_query_frontier;
use super::search_pipe_provider_facts::{ProviderGraphFactsContext, collect_provider_graph_facts};
use super::search_pipe_read_memory::read_loop_memory_selectors;
use super::search_pipe_render::{render_empty_ingest_diagnostic, render_owner_tests_frontier};
use super::search_pipe_source::collect_search_pipe_candidates;
use super::search_pipe_surfaces::default_search_surfaces;
use super::search_pipe_view::{
    SearchPipeViewRequest, print_search_pipe_view, reject_non_graph_turbo_receipt,
};
use super::search_suggest::{
    is_search_suggest, is_unsupported_search_pipeline_command,
    reject_unsupported_search_pipeline_command, run_search_suggest_command,
};

pub(super) struct FastSearchContext<'a> {
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) cache_home: &'a Path,
    pub(super) config: &'a AspConfig,
    pub(super) provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    pub(super) frontier_receipt: Option<&'a GraphTurboReceiptRequest>,
}

pub(super) fn is_asp_fast_search(args: &[String]) -> bool {
    is_search_pipe(args)
        || is_search_suggest(args)
        || is_unsupported_search_pipeline_command(args)
        || is_search_ingest(args)
        || is_search_fzf(args)
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
        return run_search_ingest_command(
            context.language_id,
            args,
            context.project_root,
            context.locator_root,
            context.config,
            context.provider_context,
            context.frontier_receipt,
        );
    }
    if is_search_fzf(args) {
        return run_search_fzf_command(
            context.language_id,
            args,
            context.project_root,
            context.locator_root,
            context.config,
            context.provider_context,
            context.frontier_receipt,
        );
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
    if is_search_owner_items_query(args) {
        return run_search_owner_items_query_command(
            context.language_id,
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

fn is_search_fzf(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("fzf"))
        && args.get(2).is_some_and(|query| !query.starts_with('-'))
        && has_supported_fast_search_view(args)
        && !args.iter().any(|arg| arg == "--json")
        && !args
            .iter()
            .any(|arg| matches!(arg.as_str(), "--query-set" | "--owner" | "--dependency"))
}

fn is_search_failure(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("failure"))
        && explicit_view(args).is_some_and(|view| matches!(view, "seeds" | "graph-turbo-request"))
        && !args.iter().any(|arg| arg == "--json" || arg == "--code")
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

fn is_search_owner_items_query(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("owner"))
        && args.iter().any(|arg| arg == "--query")
        && args.iter().any(|arg| arg == "items")
        && has_explicit_seed_view(args)
        && !args.iter().any(|arg| arg == "--json" || arg == "--code")
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
    let acquisition = collect_search_pipe_candidates(
        context.language_id,
        context.project_root,
        context.locator_root,
        &pipe_args.seed_query,
        &pipe_args.scopes,
        pipe_args.source,
        context.config,
    )?;
    let provider_facts = collect_provider_graph_facts(
        context.language_id,
        context.project_root,
        Some(&pipe_args.seed_query),
        &acquisition.candidates,
        context.config,
        context.provider_context,
    )?;
    let surfaces = default_search_surfaces();
    print_search_pipe_view(SearchPipeViewRequest {
        language_id: context.language_id,
        project_root: context.project_root,
        locator_root: context.locator_root,
        surface: "search-pipe",
        query: Some(&pipe_args.seed_query),
        candidates: &acquisition.candidates,
        pipes: &surfaces,
        source: pipe_args.source.as_str(),
        candidate_sources: &acquisition.candidate_sources,
        source_trace: &acquisition.source_trace,
        scopes: &pipe_args.scopes,
        view: &pipe_args.view,
        include_pipe_plan: true,
        provider_facts: &provider_facts,
        read_memory_selectors: &read_loop_memory_selectors(
            context.cache_home,
            context.project_root,
        ),
        frontier_receipt: context.frontier_receipt,
    })?;
    Ok(())
}

fn run_reasoning_owner_query_command(
    language_id: &str,
    args: &[String],
    project_root: &Path,
    locator_root: &Path,
    frontier_receipt: Option<&GraphTurboReceiptRequest>,
) -> Result<(), String> {
    reject_non_graph_turbo_receipt(frontier_receipt)?;
    let owner_query_args = parse_owner_query_args(args)?;
    if owner_query_args.view != "seeds" {
        return Err("search reasoning owner-query fast path supports --view seeds".to_string());
    }
    print!(
        "{}",
        render_owner_query_frontier(
            language_id,
            project_root,
            locator_root,
            &owner_query_args.owner,
            &owner_query_args.query
        )
    );
    Ok(())
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

fn run_search_owner_items_query_command(
    language_id: &str,
    args: &[String],
    project_root: &Path,
    locator_root: &Path,
    frontier_receipt: Option<&GraphTurboReceiptRequest>,
) -> Result<(), String> {
    reject_non_graph_turbo_receipt(frontier_receipt)?;
    let owner_query_args = parse_search_owner_items_query_args(args)?;
    if owner_query_args.view != "seeds" {
        return Err("search owner items fast path supports --view seeds".to_string());
    }
    print!(
        "{}",
        render_owner_query_frontier(
            language_id,
            project_root,
            locator_root,
            &owner_query_args.owner,
            &owner_query_args.query
        )
    );
    Ok(())
}

fn run_search_ingest_command(
    language_id: &str,
    args: &[String],
    project_root: &Path,
    locator_root: &Path,
    config: &AspConfig,
    provider_context: Option<&ProviderGraphFactsContext<'_>>,
    frontier_receipt: Option<&GraphTurboReceiptRequest>,
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
        print!("{}", render_empty_ingest_diagnostic(language_id));
        return Ok(());
    }
    let candidates = parse_ingest_candidates(project_root, locator_root, stdin.as_slice());
    let provider_facts = collect_provider_graph_facts(
        language_id,
        project_root,
        None,
        &candidates,
        config,
        provider_context,
    )?;
    print_search_pipe_view(SearchPipeViewRequest {
        language_id,
        project_root,
        locator_root,
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
        read_memory_selectors: &[],
        frontier_receipt,
    })?;
    Ok(())
}

fn run_search_fzf_command(
    language_id: &str,
    args: &[String],
    project_root: &Path,
    locator_root: &Path,
    config: &AspConfig,
    provider_context: Option<&ProviderGraphFactsContext<'_>>,
    frontier_receipt: Option<&GraphTurboReceiptRequest>,
) -> Result<(), String> {
    let pipe_args = parse_fzf_args(args)?;
    if !matches!(pipe_args.view.as_str(), "seeds" | "graph-turbo-request") {
        return Err(
            "search fzf fast path supports --view seeds or --view graph-turbo-request".to_string(),
        );
    }
    let candidates = collect_candidates(
        language_id,
        project_root,
        locator_root,
        &pipe_args.query,
        &pipe_args.owners,
        config,
    )?;
    let provider_facts = collect_provider_graph_facts(
        language_id,
        project_root,
        Some(&pipe_args.query),
        &candidates,
        config,
        provider_context,
    )?;
    print_search_pipe_view(SearchPipeViewRequest {
        language_id,
        project_root,
        locator_root,
        surface: "search-fzf",
        query: Some(&pipe_args.query),
        candidates: &candidates,
        pipes: &pipe_args.pipes,
        source: "finder",
        candidate_sources: &["finder".to_string()],
        source_trace: &[SearchPipeSourceTrace::new(
            "finder",
            if candidates.is_empty() {
                "empty"
            } else {
                "used"
            },
            candidates.len(),
            usize::from(candidates.is_empty()),
            candidates.len(),
        )],
        scopes: &pipe_args.owners,
        view: &pipe_args.view,
        include_pipe_plan: false,
        provider_facts: &provider_facts,
        read_memory_selectors: &[],
        frontier_receipt,
    })?;
    Ok(())
}

fn run_search_failure_command(
    language_id: &str,
    args: &[String],
    project_root: &Path,
    locator_root: &Path,
    cache_home: &Path,
    config: &AspConfig,
) -> Result<(), String> {
    let failure_args = parse_failure_args(args)?;
    if !matches!(failure_args.view.as_str(), "seeds" | "graph-turbo-request") {
        return Err(
            "search failure fast path supports --view seeds or --view graph-turbo-request"
                .to_string(),
        );
    }
    let message = if failure_args.from_last_check {
        read_last_check_output(cache_home)?
    } else {
        failure_args
            .message
            .ok_or_else(|| "search failure requires --message or --from-last-check".to_string())?
    };
    if message.trim().is_empty() {
        return Err("search failure requires non-empty failure text".to_string());
    }
    let candidate_query = failure_candidate_query(&message);
    let candidates = collect_candidates(
        language_id,
        project_root,
        locator_root,
        &candidate_query,
        &[],
        config,
    )?;
    let rendered = if failure_args.view == "graph-turbo-request" {
        render_failure_graph_turbo_request(
            language_id,
            project_root,
            locator_root,
            &message,
            &candidates,
        )?
    } else {
        render_failure_frontier(
            language_id,
            project_root,
            locator_root,
            &message,
            &candidates,
        )?
    };
    print!("{rendered}");
    Ok(())
}

fn read_last_check_output(cache_home: &Path) -> Result<String, String> {
    let path = cache_home
        .join("agent-semantic-protocol")
        .join("last-check-output.txt");
    fs::read_to_string(&path).map_err(|error| {
        format!(
            "search failure --from-last-check could not read {}: {error}",
            path.display()
        )
    })
}

fn failure_candidate_query(message: &str) -> String {
    let mut terms = Vec::new();
    for token in message
        .split(|character: char| !failure_token_character(character))
        .filter(|token| !token.is_empty())
    {
        if token.contains("::") {
            if let Some(last) = token.rsplit("::").find(|part| !part.is_empty()) {
                push_failure_candidate_term(&mut terms, last);
            }
        } else {
            push_failure_candidate_term(&mut terms, token);
        }
    }
    if terms.is_empty() {
        return message.to_string();
    }
    terms.join(" ")
}

fn push_failure_candidate_term(terms: &mut Vec<String>, token: &str) {
    let token = token.trim_matches([':', '.', ',', ';', '(', ')', '[', ']']);
    let lower = token.to_ascii_lowercase();
    if token.len() < 4
        || failure_candidate_stop_word(&lower)
        || !(token.contains('_') || token.contains('-'))
    {
        return;
    }
    if !terms.iter().any(|term| term == token) {
        terms.push(token.to_string());
    }
}

fn failure_candidate_stop_word(token: &str) -> bool {
    matches!(
        token,
        "expected"
            | "actual"
            | "failure"
            | "failed"
            | "panic"
            | "error"
            | "status"
            | "stdout"
            | "stderr"
            | "left"
            | "right"
            | "pass"
            | "fail"
            | "hit"
            | "miss"
            | "observed"
            | "unknown"
            | "request_fingerprint"
            | "file_hash"
    )
}

fn failure_token_character(character: char) -> bool {
    character == '_' || character == '-' || character == ':' || character.is_ascii_alphanumeric()
}
