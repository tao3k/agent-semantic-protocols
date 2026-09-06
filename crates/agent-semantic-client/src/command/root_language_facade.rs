// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Runtime-owned workspace Search planner entrypoint.

use std::path::PathBuf;

pub(crate) async fn run_workspace_search_playbook(args: &[String]) -> Result<(), String> {
    let mut parser_args = vec!["search".to_owned()];
    parser_args.extend_from_slice(args);
    let request = agent_semantic_search::parse_progressive_search_playbook_args(&parser_args)
        .map_err(|error| error.to_string())?;
    let contract_query = matches!(
        request,
        agent_semantic_search::ProgressiveSearchPlaybookRequest::ContractQuery { .. }
    );
    let (languages, documents, workspace, fd, rg, tantivy, syntax, graph, clause_order) =
        match request {
        agent_semantic_search::ProgressiveSearchPlaybookRequest::ContractQuery {
            languages,
            documents,
            workspace,
        } => (
            languages, documents, workspace, None, None, None, None, None, None,
        ),
        agent_semantic_search::ProgressiveSearchPlaybookRequest::Execute {
            languages,
            documents,
            workspace,
            fd,
            rg,
            tantivy,
            syntax,
            graph,
            clause_order,
        } => (
            languages,
            documents,
            workspace,
            (!fd.is_empty()).then_some(fd),
            (!rg.is_empty()).then_some(rg),
            (!tantivy.is_empty()).then_some(tantivy),
            (!syntax.is_empty()).then(|| {
                syntax
                    .into_iter()
                    .map(|block| {
                        agent_semantic_client_protocol::AspClientSearchPlaybookSyntaxBlock {
                            producer: block.producer,
                            argv: block.argv,
                        }
                    })
                    .collect()
            }),
            (!graph.is_empty()).then(|| {
                graph
                    .into_iter()
                    .map(|block| {
                        agent_semantic_client_protocol::AspClientSearchPlaybookGraphBlock {
                            language: block.language,
                            argv: block.argv,
                        }
                    })
                    .collect()
            }),
            Some(
                clause_order
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
                                agent_semantic_search::SearchPlaybookClauseAxis::Graph => {
                                    agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::Graph
                                }
                            },
                            block_index: clause.block_index,
                        }
                    })
                    .collect(),
            ),
        ),
    };

    let project_root = resolve_playbook_workspace(workspace.as_deref())?;
    let local_acquisition_only = !contract_query
        && syntax.is_none()
        && graph.is_none()
        && (fd.is_some() || rg.is_some() || tantivy.is_some());
    if local_acquisition_only {
        let receipt = agent_semantic_search::execute_local_search_playbook_acquisition(
            &project_root,
            fd.as_deref().unwrap_or_default(),
            rg.as_deref().unwrap_or_default(),
            tantivy.as_deref().unwrap_or_default(),
        )
        .await?;
        println!("{}", render_local_search_acquisition(&receipt));
        return Ok(());
    }

    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    #[cfg(unix)]
    let client = crate::AspClient::new_from_host_capability(state_home, &project_root)?;
    #[cfg(not(unix))]
    let client = crate::AspClient::new(state_home, &project_root);
    let frame = client
        .dispatch_method(
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
                    graph,
                    clause_order,
                },
            )
            .map_err(|error| format!("encode Search playbook request: {error}"))?,
        )
        .await?;
    if contract_query {
        println!(
            "{}",
            crate::projection_presentation::render_search_playbook_contract_response(&frame)?
        );
    } else {
        println!(
            "{}",
            crate::projection_presentation::render_workspace_search_playbook_result(&frame)?
        );
    }
    Ok(())
}

pub(crate) async fn run_workspace_query(args: &[String]) -> Result<(), String> {
    let mut parser_args = vec!["query".to_owned()];
    parser_args.extend_from_slice(args);
    let request = agent_semantic_search::parse_progressive_query_args(&parser_args)?;
    match request {
        agent_semantic_search::ProgressiveQueryRequest::Selector {
            selector,
            projection,
            output_format,
            workspace,
        } => {
            let language_id = selector
                .split_once("://")
                .map(|(language_id, _)| language_id)
                .filter(|language_id| !language_id.is_empty())
                .ok_or_else(|| {
                    "query selector must carry its registered producer as `<producer>://...`"
                        .to_owned()
                })?;
            let project_root = resolve_query_workspace(workspace.as_deref())?;
            agent_semantic_client::execute_language_command(
                agent_semantic_client::LanguageCommandRequest {
                    language_id: agent_semantic_client::LanguageId::new(language_id),
                    operation: agent_semantic_client::LanguageCommandOperation::ExactQuery(
                        agent_semantic_client_protocol::AspClientExactQueryRequest {
                            schema_id: "agent.semantic-protocols.asp-client-exact-query-request"
                                .to_owned(),
                            schema_version: "1".to_owned(),
                            selector,
                            projection,
                        },
                    ),
                    project_root,
                    machine_readable: output_format
                        == agent_semantic_search::QueryOutputFormat::Json,
                },
            )
            .await
        }
        agent_semantic_search::ProgressiveQueryRequest::Syntax {
            languages,
            documents,
            workspace,
            syntax,
            projection,
            output_format,
        } => {
            let project_root = resolve_query_workspace(workspace.as_deref())?;
            let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
            #[cfg(unix)]
            let client = crate::AspClient::new_from_host_capability(state_home, &project_root)?;
            #[cfg(not(unix))]
            let client = crate::AspClient::new(state_home, &project_root);
            let frame = client
                .dispatch_method(
                    agent_semantic_client_protocol::WORKSPACE_SYNTAX_QUERY_METHOD.to_owned(),
                    serde_json::to_value(
                        agent_semantic_client_protocol::AspClientWorkspaceSyntaxQueryRequest {
                            schema_id: "agent.semantic-protocols.asp-client-workspace-syntax-query-request".to_owned(),
                            schema_version: "1".to_owned(),
                            languages,
                            documents,
                            workspace,
                            syntax: syntax.into_iter().map(|block| {
                                agent_semantic_client_protocol::AspClientSearchPlaybookSyntaxBlock {
                                    producer: block.producer,
                                    argv: block.argv,
                                }
                            }).collect(),
                            projection,
                        },
                    )
                    .map_err(|error| format!("encode workspace syntax Query request: {error}"))?,
                )
                .await?;
            let presentation = if output_format == agent_semantic_search::QueryOutputFormat::Json {
                crate::projection_presentation::ProjectionPresentation::MachineJson
            } else {
                crate::projection_presentation::ProjectionPresentation::Text
            };
            println!(
                "{}",
                crate::projection_presentation::render_workspace_syntax_query_response(
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

fn render_local_search_acquisition(
    receipt: &agent_semantic_search::LocalSearchPlaybookAcquisitionReceipt,
) -> String {
    let mut output = format!(
        "#+title: ASP Search Playbook Layer One Evidence\n#+property: SCHEMA {}\n#+property: SCHEMA_VERSION {}\n#+property: CONTENT_GENERATION {}\n#+property: SELECTOR_AUTHORITY none\n\n* Candidate owner evidence\n",
        receipt.schema_id, receipt.schema_version, receipt.content_generation_digest
    );
    if receipt.candidate_owner_paths.is_empty() {
        output.push_str("No lexical owner candidates matched.\n");
        return output;
    }
    for owner in &receipt.candidate_owner_paths {
        let mut axes = Vec::new();
        if receipt
            .fd
            .candidate_owner_paths
            .binary_search(owner)
            .is_ok()
        {
            axes.push("fd");
        }
        if receipt
            .rg
            .candidate_owner_paths
            .binary_search(owner)
            .is_ok()
        {
            axes.push("rg");
        }
        if receipt
            .tantivy
            .candidate_owner_paths
            .binary_search(owner)
            .is_ok()
        {
            axes.push("tantivy");
        }
        output.push_str(&format!(
            "** [[file:{owner}][{owner}]]\n:PROPERTIES:\n:ACQUISITION_AXES: {}\n:END:\n",
            axes.join(" ")
        ));
    }
    output.push_str(
        "\n* Next action\nUse parser-owned syntax evidence to obtain canonical selectors before `asp query --selector`.\n",
    );
    output
}
