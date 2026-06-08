//! Graph-turbo view rendering for ASP-owned search pipelines.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::graph::{
    GraphTurboReceiptCapture, GraphTurboReceiptRequest, render_graph_turbo_packet,
    write_graph_turbo_receipt,
};
use super::search_pipe_graph_turbo::render_graph_turbo_request;
use super::search_pipe_plan::{render_primary_frontier_actions_only, render_search_pipe_plan};
use super::search_pipe_provider_facts::ProviderGraphFacts;
use super::search_pipe_render::{Candidate, render_ingest_frontier};

pub(super) struct SearchPipeViewRequest<'a> {
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) query: Option<&'a str>,
    pub(super) candidates: &'a [Candidate],
    pub(super) pipes: &'a [String],
    pub(super) scopes: &'a [PathBuf],
    pub(super) view: &'a str,
    pub(super) include_pipe_plan: bool,
    pub(super) provider_facts: &'a ProviderGraphFacts,
    pub(super) read_memory_selectors: &'a [String],
    pub(super) frontier_receipt: Option<&'a GraphTurboReceiptRequest>,
}

pub(super) fn print_search_pipe_view(request: SearchPipeViewRequest<'_>) -> Result<(), String> {
    let SearchPipeViewRequest {
        language_id,
        project_root,
        locator_root,
        query,
        candidates,
        pipes,
        scopes,
        view,
        include_pipe_plan,
        provider_facts,
        read_memory_selectors,
        frontier_receipt,
    } = request;
    match view {
        "graph-turbo-request" => {
            let request = render_graph_turbo_request(
                language_id,
                query,
                candidates,
                pipes,
                provider_facts,
                read_memory_selectors,
            )?;
            write_fast_search_frontier_receipt(
                frontier_receipt,
                language_id,
                query,
                request.as_bytes(),
            )?;
            print!("{request}");
        }
        "seeds" => {
            let request = render_graph_turbo_request(
                language_id,
                query,
                candidates,
                pipes,
                provider_facts,
                read_memory_selectors,
            )?;
            write_fast_search_frontier_receipt(
                frontier_receipt,
                language_id,
                query,
                request.as_bytes(),
            )?;
            let mut ranked_compact = None;
            if let Some(output) = render_graph_turbo_packet(request.as_bytes())? {
                ranked_compact = std::str::from_utf8(output.as_ref())
                    .ok()
                    .map(str::to_string);
                if include_pipe_plan {
                    if let Some(compact) = ranked_compact.as_deref() {
                        print!("{}", render_primary_frontier_actions_only(compact));
                    } else {
                        io::stdout().write_all(output.as_ref()).map_err(|error| {
                            format!("failed to write asp-graph-turbo stdout: {error}")
                        })?;
                    }
                } else {
                    io::stdout().write_all(output.as_ref()).map_err(|error| {
                        format!("failed to write asp-graph-turbo stdout: {error}")
                    })?;
                }
            } else {
                print!("{}", render_ingest_frontier(candidates, pipes));
            }
            if include_pipe_plan && let Some(query) = query {
                print!(
                    "{}",
                    render_search_pipe_plan(
                        language_id,
                        project_root,
                        locator_root,
                        scopes,
                        query,
                        candidates,
                        ranked_compact.as_deref(),
                    )
                );
            }
        }
        _ => {
            reject_non_graph_turbo_receipt(frontier_receipt)?;
            print!("{}", render_ingest_frontier(candidates, pipes));
            if include_pipe_plan && let Some(query) = query {
                print!(
                    "{}",
                    render_search_pipe_plan(
                        language_id,
                        project_root,
                        locator_root,
                        scopes,
                        query,
                        candidates,
                        None,
                    )
                );
            }
        }
    }
    Ok(())
}

pub(super) fn reject_non_graph_turbo_receipt(
    frontier_receipt: Option<&GraphTurboReceiptRequest>,
) -> Result<(), String> {
    if frontier_receipt.is_some() {
        return Err(
            "--frontier-receipt-out is supported only for graph-turbo frontier search commands"
                .to_string(),
        );
    }
    Ok(())
}

fn write_fast_search_frontier_receipt(
    frontier_receipt: Option<&GraphTurboReceiptRequest>,
    language_id: &str,
    query: Option<&str>,
    request_bytes: &[u8],
) -> Result<(), String> {
    let Some(receipt) = frontier_receipt else {
        return Ok(());
    };
    let token = frontier_receipt_token(language_id, query);
    write_graph_turbo_receipt(
        request_bytes,
        &GraphTurboReceiptCapture {
            out_path: receipt.out_path.as_path(),
            receipt_id: &format!("asp.search-frontier.{token}"),
            task_fingerprint: &format!("task:asp-fast-search:{token}"),
            command_fingerprint: &format!("command:asp-fast-search:{token}"),
            capture_source: "asp fast search",
            extra_args: &receipt.extra_args,
        },
    )
}

fn frontier_receipt_token(language_id: &str, query: Option<&str>) -> String {
    let mut token = format!("{}-{}", language_id, query.unwrap_or("frontier"))
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while token.contains("--") {
        token = token.replace("--", "-");
    }
    let token = token.trim_matches('-');
    if token.is_empty() {
        return "frontier".to_string();
    }
    token.chars().take(80).collect()
}
