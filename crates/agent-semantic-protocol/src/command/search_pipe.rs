//! ASP-owned search pipeline wrapper.

use std::fs;
use std::path::{Path, PathBuf};

use super::graph::GraphTurboReceiptRequest;
use super::search_config::AspConfig;
use super::search_failure_render::{render_failure_frontier, render_failure_graph_turbo_request};
use super::search_pipe_candidates::{
    collect_candidates, parse_ingest_candidates, read_piped_stdin,
};
use super::search_pipe_provider_facts::{ProviderGraphFactsContext, collect_provider_graph_facts};
use super::search_pipe_read_memory::read_loop_memory_selectors;
use super::search_pipe_render::{
    render_empty_ingest_diagnostic, render_owner_query_frontier, render_owner_tests_frontier,
};
use super::search_pipe_view::{
    SearchPipeViewRequest, print_search_pipe_view, reject_non_graph_turbo_receipt,
};
use super::search_suggest::{
    is_search_suggest, is_unsupported_search_pipeline_command,
    reject_unsupported_search_pipeline_command, run_search_suggest_command,
};

#[derive(Debug, Eq, PartialEq)]
struct SearchPipeArgs {
    query: String,
    pipes: Vec<String>,
    owners: Vec<PathBuf>,
    view: String,
}

#[derive(Debug, Eq, PartialEq)]
struct OwnerQueryArgs {
    owner: PathBuf,
    query: String,
    view: String,
}

#[derive(Debug, Eq, PartialEq)]
struct OwnerOnlyArgs {
    owner: PathBuf,
    view: String,
}

#[derive(Debug, Eq, PartialEq)]
struct IngestArgs {
    pipes: Vec<String>,
    view: String,
}

#[derive(Debug, Eq, PartialEq)]
struct FailureArgs {
    message: Option<String>,
    from_last_check: bool,
    view: String,
}

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
    let candidates = collect_candidates(
        context.language_id,
        context.project_root,
        context.locator_root,
        &pipe_args.query,
        &pipe_args.owners,
        context.config,
    )?;
    let provider_facts = collect_provider_graph_facts(
        context.language_id,
        context.project_root,
        Some(&pipe_args.query),
        &candidates,
        context.config,
        context.provider_context,
    )?;
    print_search_pipe_view(SearchPipeViewRequest {
        language_id: context.language_id,
        project_root: context.project_root,
        locator_root: context.locator_root,
        query: Some(&pipe_args.query),
        candidates: &candidates,
        pipes: &pipe_args.pipes,
        scopes: &pipe_args.owners,
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
        query: None,
        candidates: &candidates,
        pipes: &ingest_args.pipes,
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
        query: Some(&pipe_args.query),
        candidates: &candidates,
        pipes: &pipe_args.pipes,
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

fn parse_search_pipe_args(args: &[String]) -> Result<SearchPipeArgs, String> {
    if !is_search_pipe(args) {
        return Err("expected search pipe command".to_string());
    }
    let query = args
        .get(2)
        .filter(|query| !query.starts_with('-'))
        .ok_or_else(|| "search pipe requires a query".to_string())?
        .clone();
    let mut pipes = Vec::new();
    let mut owners = Vec::new();
    let mut view = "seeds".to_string();
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--pipe" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--pipe requires a value".to_string())?;
                pipes.extend(split_csv(value));
                index += 2;
            }
            "--owners" | "--owner" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{} requires a value", args[index]))?;
                owners.extend(split_csv(value).into_iter().map(PathBuf::from));
                index += 2;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown search pipe option: {value}"));
            }
            value => {
                owners.push(PathBuf::from(value));
                index += 1;
            }
        }
    }
    if view == "commands" {
        return Err(
            "search pipe --view commands moved to search suggest --view commands".to_string(),
        );
    }
    if !matches!(view.as_str(), "seeds" | "graph-turbo-request") {
        return Err("search pipe supports --view seeds or --view graph-turbo-request".to_string());
    }
    if pipes.is_empty() {
        pipes.extend(["items".to_string(), "tests".to_string()]);
    }
    Ok(SearchPipeArgs {
        query,
        pipes,
        owners,
        view,
    })
}

fn parse_owner_query_args(args: &[String]) -> Result<OwnerQueryArgs, String> {
    if !is_reasoning_owner_query(args) {
        return Err("expected search reasoning owner-query command".to_string());
    }
    let mut owner = None;
    let mut query = None;
    let mut view = "seeds".to_string();
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--owner" => {
                owner = Some(PathBuf::from(
                    args.get(index + 1)
                        .ok_or_else(|| "--owner requires a value".to_string())?,
                ));
                index += 2;
            }
            "--query" => {
                query = Some(
                    args.get(index + 1)
                        .ok_or_else(|| "--query requires a value".to_string())?
                        .clone(),
                );
                index += 2;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            value if value.starts_with('-') => {
                return Err(format!(
                    "unknown search reasoning owner-query option: {value}"
                ));
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(OwnerQueryArgs {
        owner: owner.ok_or_else(|| "search reasoning owner-query requires --owner".to_string())?,
        query: query.ok_or_else(|| "search reasoning owner-query requires --query".to_string())?,
        view,
    })
}

fn parse_owner_only_args(args: &[String], profile: &str) -> Result<OwnerOnlyArgs, String> {
    let mut owner = None;
    let mut view = "seeds".to_string();
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--owner" => {
                owner = Some(PathBuf::from(
                    args.get(index + 1)
                        .ok_or_else(|| "--owner requires a value".to_string())?,
                ));
                index += 2;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            value if value.starts_with('-') => {
                return Err(format!(
                    "unknown search reasoning {profile} option: {value}"
                ));
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(OwnerOnlyArgs {
        owner: owner.ok_or_else(|| format!("search reasoning {profile} requires --owner"))?,
        view,
    })
}

fn parse_search_owner_items_query_args(args: &[String]) -> Result<OwnerQueryArgs, String> {
    let owner = args
        .get(2)
        .filter(|owner| !owner.starts_with('-'))
        .ok_or_else(|| "search owner requires an owner path".to_string())?;
    let mut query = None;
    let mut view = "seeds".to_string();
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--query" => {
                query = Some(
                    args.get(index + 1)
                        .ok_or_else(|| "--query requires a value".to_string())?
                        .clone(),
                );
                index += 2;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(OwnerQueryArgs {
        owner: PathBuf::from(owner),
        query: query.ok_or_else(|| "search owner items requires --query".to_string())?,
        view,
    })
}

fn parse_ingest_args(args: &[String]) -> Result<IngestArgs, String> {
    if !is_search_ingest(args) {
        return Err("expected search ingest command".to_string());
    }
    let mut pipes = Vec::new();
    let mut view = "seeds".to_string();
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown search ingest option: {value}"));
            }
            value => {
                pipes.push(value.to_string());
                index += 1;
            }
        }
    }
    if pipes.is_empty() {
        pipes.extend(["items".to_string(), "tests".to_string()]);
    }
    Ok(IngestArgs { pipes, view })
}

fn parse_fzf_args(args: &[String]) -> Result<SearchPipeArgs, String> {
    if !is_search_fzf(args) {
        return Err("expected search fzf command".to_string());
    }
    let query = args
        .get(2)
        .filter(|query| !query.starts_with('-'))
        .ok_or_else(|| "search fzf requires a query".to_string())?
        .clone();
    let mut pipes = Vec::new();
    let mut view = "seeds".to_string();
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            "--query-set" | "--owner" | "--dependency" => {
                return Err(format!(
                    "search fzf fast path does not support {}",
                    args[index]
                ));
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown search fzf option: {value}"));
            }
            value => {
                pipes.push(value.to_string());
                index += 1;
            }
        }
    }
    if pipes.is_empty() {
        pipes.extend(["items".to_string(), "tests".to_string()]);
    }
    Ok(SearchPipeArgs {
        query,
        pipes,
        owners: Vec::new(),
        view,
    })
}

fn parse_failure_args(args: &[String]) -> Result<FailureArgs, String> {
    if !is_search_failure(args) {
        return Err("expected search failure command".to_string());
    }
    let mut message = None;
    let mut positional = Vec::new();
    let mut from_last_check = false;
    let mut view = "seeds".to_string();
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--from-last-check" => {
                from_last_check = true;
                index += 1;
            }
            "--message" => {
                message = Some(
                    args.get(index + 1)
                        .ok_or_else(|| "--message requires a value".to_string())?
                        .clone(),
                );
                index += 2;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown search failure option: {value}"));
            }
            "." => {
                index += 1;
            }
            value => {
                positional.push(value.to_string());
                index += 1;
            }
        }
    }
    if message.is_none() && !positional.is_empty() {
        message = Some(positional.join(" "));
    }
    if from_last_check && message.is_some() {
        return Err(
            "search failure accepts either --from-last-check or failure text, not both".to_string(),
        );
    }
    if !from_last_check && message.is_none() {
        return Err("search failure requires --message or --from-last-check".to_string());
    }
    Ok(FailureArgs {
        message,
        from_last_check,
        view,
    })
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

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
