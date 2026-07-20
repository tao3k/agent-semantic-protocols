//! Failure-oriented `search pipe` command handling.

use std::fs;
use std::path::Path;

use agent_semantic_search::{
    SearchPipeFailureAcquisitionRequest, collect_search_pipe_failure_acquisition,
};

use super::search_config::AspConfig;
use super::search_failure_render::{render_failure_frontier, render_failure_graph_turbo_request};
use super::search_pipe_args::parse_failure_args;
use super::search_pipe_candidates::PIPE_CANDIDATE_LINE_LIMIT;
use super::search_pipe_model::Candidate;

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
    let current_snapshot =
        agent_semantic_client::source_index::current_source_index_snapshot(project_root)?;
    let acquisition =
        collect_search_pipe_failure_acquisition(SearchPipeFailureAcquisitionRequest {
            language_id,
            project_root,
            locator_root,
            message: &message,
            ignore_dirs: &config.search.ignore_dirs,
            include_hidden_dirs: &config.search.include_hidden_dirs,
            limit: PIPE_CANDIDATE_LINE_LIMIT,
            base_snapshot: &current_snapshot.workspace_snapshot,
            provider_digest: &current_snapshot.source_snapshot.provider_digest,
        })?;
    let candidates = acquisition
        .candidates
        .into_iter()
        .map(Candidate::from)
        .collect::<Vec<_>>();
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
