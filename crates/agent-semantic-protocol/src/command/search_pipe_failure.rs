//! Failure-oriented `search pipe` command handling.

use std::fs;
use std::path::Path;

use super::search_config::AspConfig;
use super::search_failure_render::{render_failure_frontier, render_failure_graph_turbo_request};
use super::search_pipe_args::parse_failure_args;
use super::search_pipe_candidates::collect_candidates;

pub(super) fn run_search_failure_command(
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
