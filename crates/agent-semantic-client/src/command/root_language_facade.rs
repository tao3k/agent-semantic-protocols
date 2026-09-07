// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime-owned workspace Search Playbook entrypoint.

use std::path::PathBuf;

pub(crate) async fn run_workspace_search_playbook(args: &[String]) -> Result<(), String> {
    let launcher_started = tokio::time::Instant::now();
    let mut parser_args = vec!["search".to_owned()];
    parser_args.extend_from_slice(args);
    let request = agent_semantic_search::parse_progressive_search_playbook_args(&parser_args)
        .map_err(|error| error.to_string())?;
    let agent_semantic_search::ProgressiveSearchPlaybookRequest {
        languages,
        documents,
        workspace,
        fd,
        rg,
        tantivy,
        syntax,
        native_syntax,
        graph,
        clause_order,
    } = request;
    let fd = (!fd.is_empty()).then_some(fd);
    let rg = (!rg.is_empty()).then_some(rg);
    let tantivy = (!tantivy.is_empty()).then_some(tantivy);
    let syntax = (!syntax.is_empty()).then(|| {
        syntax
            .into_iter()
            .map(
                |block| agent_semantic_client_protocol::AspClientSearchPlaybookSyntaxBlock {
                    producer: block.producer,
                    argv: block.argv,
                },
            )
            .collect()
    });
    let native_syntax = (!native_syntax.is_empty()).then_some(native_syntax);
    let graph = (!graph.is_empty()).then(|| {
        graph
            .into_iter()
            .map(
                |block| agent_semantic_client_protocol::AspClientSearchPlaybookGraphBlock {
                    language: block.language,
                    argv: block.argv,
                },
            )
            .collect()
    });
    let clause_order = clause_order
                    .into_iter()
                    .map(|clause| {
                        agent_semantic_client_protocol::AspClientSearchPlaybookClauseRef {
                            axis: match clause.axis {
                                agent_semantic_search::SearchPlaybookClauseAxis::Fd => {
                                    agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::Fd
                                }
                                agent_semantic_search::SearchPlaybookClauseAxis::Rg => {
                                    agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::Rg
                                }
                                agent_semantic_search::SearchPlaybookClauseAxis::Tantivy => {
                                    agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::Tantivy
                                }
                                agent_semantic_search::SearchPlaybookClauseAxis::Syntax => {
                                    agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::Syntax
                                }
                                agent_semantic_search::SearchPlaybookClauseAxis::NativeSyntax => {
                                    agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::NativeSyntax
                                }
                                agent_semantic_search::SearchPlaybookClauseAxis::Graph => {
                                    agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::Graph
                                }
                            },
                            block_index: clause.block_index,
                        }
                    })
                    .collect();

    let project_root = resolve_playbook_workspace(workspace.as_deref())?;
    let launcher_elapsed_micros = launcher_started
        .elapsed()
        .as_micros()
        .min(u128::from(u64::MAX)) as u64;

    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    #[cfg(unix)]
    let client = crate::AspClient::new_from_host_capability(state_home, &project_root)?;
    #[cfg(not(unix))]
    let client = crate::AspClient::new(state_home, &project_root);
    let frame = client
        .dispatch_playbook_method(
            agent_semantic_client_protocol::WORKSPACE_SEARCH_PLAYBOOK_METHOD.to_owned(),
            serde_json::to_value(
                agent_semantic_client_protocol::AspClientWorkspaceSearchPlaybookRequest {
                    schema_id:
                        "agent.semantic-protocols.asp-client-workspace-search-playbook-request"
                            .to_owned(),
                    schema_version: "1".to_owned(),
                    languages,
                    documents,
                    workspace,
                    fd,
                    rg,
                    tantivy,
                    syntax,
                    native_syntax,
                    graph,
                    clause_order,
                },
            )
            .map_err(|error| format!("encode Search playbook request: {error}"))?,
            launcher_elapsed_micros,
        )
        .await?;
    println!(
        "{}",
        crate::projection_presentation::render_workspace_search_playbook_gql(&frame)?
    );
    Ok(())
}

pub(crate) async fn run_workspace_query(args: &[String]) -> Result<(), String> {
    let launcher_started = tokio::time::Instant::now();
    let mut parser_args = vec!["query".to_owned()];
    parser_args.extend_from_slice(args);
    let request = agent_semantic_search::parse_progressive_query_args(&parser_args)?;
    match request {
        agent_semantic_search::ProgressiveQueryRequest::Selector {
            selectors,
            projection,
            output_format,
            workspace,
        } => {
            let project_root = resolve_query_workspace(workspace.as_deref())?;
            let launcher_elapsed_micros = launcher_started
                .elapsed()
                .as_micros()
                .min(u128::from(u64::MAX)) as u64;
            let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
            #[cfg(unix)]
            let client = crate::AspClient::new_from_host_capability(state_home, &project_root)?;
            #[cfg(not(unix))]
            let client = crate::AspClient::new(state_home, &project_root);
            let frame = client
                .dispatch_playbook_method(
                    agent_semantic_client_protocol::WORKSPACE_QUERY_PLAYBOOK_METHOD.to_owned(),
                    serde_json::to_value(
                        agent_semantic_client_protocol::AspClientWorkspaceQueryPlaybookRequest {
                            schema_id: "agent.semantic-protocols.asp-client-workspace-query-playbook-request".to_owned(),
                            schema_version: "1".to_owned(),
                            selectors,
                            projection,
                        },
                    )
                    .map_err(|error| format!("encode Query Playbook request: {error}"))?,
                    launcher_elapsed_micros,
                )
                .await?;
            let presentation = if output_format == agent_semantic_search::QueryOutputFormat::Json {
                crate::projection_presentation::ProjectionPresentation::MachineJson
            } else {
                crate::projection_presentation::ProjectionPresentation::Text
            };
            println!(
                "{}",
                crate::projection_presentation::render_workspace_query_playbook_response(
                    &frame,
                    presentation,
                )?
            );
            Ok(())
        }
    }
}

fn resolve_playbook_workspace(workspace: Option<&str>) -> Result<PathBuf, String> {
    let workspace = match workspace {
        None | Some(".") => std::env::current_dir()
            .map_err(|error| format!("failed to resolve current project directory: {error}"))?,
        Some(workspace) => PathBuf::from(workspace),
    };
    if !workspace.is_dir() {
        return Err(format!(
            "search playbook workspace must be a directory: {}",
            workspace.display()
        ));
    }
    std::fs::canonicalize(&workspace).map_err(|error| {
        format!(
            "failed to canonicalize Search playbook workspace {}: {error}",
            workspace.display()
        )
    })
}

fn resolve_query_workspace(workspace: Option<&str>) -> Result<PathBuf, String> {
    match workspace {
        None | Some(".") => resolve_playbook_workspace(workspace),
        Some(workspace) => std::fs::canonicalize(workspace)
            .map_err(|error| format!("failed to resolve Query workspace {workspace}: {error}")),
    }
}
