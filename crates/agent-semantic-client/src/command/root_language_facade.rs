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
        language,
        documents,
        workspace,
        rg,
        tantivy,
        syntax,
        native_syntax,
        graph,
        clause_order,
    } = request;
    let rg = (!rg.is_empty()).then_some(rg);
    let tantivy = (!tantivy.is_empty()).then_some(tantivy);
    let syntax_sources = syntax;
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

    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    let project_root = resolve_registered_workspace(workspace.as_deref(), &state_home).await?;
    let launcher_elapsed_micros = launcher_started
        .elapsed()
        .as_micros()
        .min(u128::from(u64::MAX)) as u64;

    #[cfg(unix)]
    let client = crate::AspClient::new_from_host_capability(state_home, &project_root)?;
    #[cfg(not(unix))]
    let client = crate::AspClient::new(state_home, &project_root);
    let client = client.admit_runtime_workspace("Search Playbook").await?;
    let syntax = compile_resident_syntax_blocks(&client, syntax_sources).await?;
    let frame = client
        .dispatch_playbook_method(
            agent_semantic_client_protocol::WORKSPACE_SEARCH_PLAYBOOK_METHOD.to_owned(),
            serde_json::to_value(
                agent_semantic_client_protocol::AspClientWorkspaceSearchPlaybookRequest {
                    schema_id:
                        "agent.semantic-protocols.asp-client-workspace-search-playbook-request"
                            .to_owned(),
                    schema_version: "1".to_owned(),
                    language,
                    documents,
                    workspace,
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

async fn compile_resident_syntax_blocks(
    client: &crate::AspClient,
    blocks: Vec<agent_semantic_search::ProducerNativeBlock>,
) -> Result<Option<Vec<agent_semantic_client_protocol::AspClientSearchPlaybookSyntaxBlock>>, String>
{
    if blocks.is_empty() {
        return Ok(None);
    }
    let mut compiled = Vec::with_capacity(blocks.len());
    for block in blocks {
        let [query_source] = block.argv.as_slice() else {
            return Err("syntax requires exactly one enhanced Tree-sitter Query string".to_owned());
        };
        let frame = client
            .dispatch_method(
                agent_semantic_client_protocol::WORKSPACE_SYNTAX_PLAN_CONTEXT_METHOD.to_owned(),
                serde_json::to_value(
                    agent_semantic_client_protocol::AspClientWorkspaceSyntaxPlanContextRequest {
                        schema_id:
                            "agent.semantic-protocols.asp-client-workspace-syntax-plan-context-request"
                                .to_owned(),
                        schema_version: "1".to_owned(),
                        producer: block.producer.clone(),
                    },
                )
                .map_err(|error| format!("encode syntax plan context request: {error}"))?,
            )
            .await?;
        let agent_semantic_client_protocol::ClientFrame::Response {
            outcome: agent_semantic_client_protocol::ClientOutcome::Ready,
            result: Some(result),
            error: None,
            ..
        } = frame
        else {
            return Err(format!(
                "Runtime rejected enhanced syntax plan context read: {frame:?}"
            ));
        };
        let response: agent_semantic_client_protocol::AspClientWorkspaceSyntaxPlanContextResponse =
            serde_json::from_value(result)
                .map_err(|error| format!("decode syntax plan context response: {error}"))?;
        response.validate()?;
        if response.schema_id
            != "agent.semantic-protocols.asp-client-workspace-syntax-plan-context-response"
            || response.schema_version != "1"
            || response.capability.language_id != block.producer
        {
            return Err("Runtime syntax plan context response identity mismatch".to_owned());
        }
        let plan = agent_semantic_tree_sitter::compile_resident_syntax_plan(
            query_source,
            &response.generation_digest,
            &response.capability,
        )?;
        compiled.push(
            agent_semantic_client_protocol::AspClientSearchPlaybookSyntaxBlock {
                producer: block.producer,
                plan,
            },
        );
    }
    Ok(Some(compiled))
}

pub(crate) async fn run_workspace_query(args: &[String]) -> Result<(), String> {
    let launcher_started = tokio::time::Instant::now();
    let mut parser_args = vec!["query".to_owned()];
    parser_args.extend_from_slice(args);
    let request = agent_semantic_search::parse_progressive_query_args(&parser_args)?;
    match request {
        agent_semantic_search::ProgressiveQueryRequest::Selector {
            language,
            documents,
            selectors,
            projection,
            output_format,
            workspace,
        } => {
            let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
            let project_root =
                resolve_registered_workspace(workspace.as_deref(), &state_home).await?;
            let launcher_elapsed_micros = launcher_started
                .elapsed()
                .as_micros()
                .min(u128::from(u64::MAX)) as u64;
            #[cfg(unix)]
            let client = crate::AspClient::new_from_host_capability(state_home, &project_root)?;
            #[cfg(not(unix))]
            let client = crate::AspClient::new(state_home, &project_root);
            let client = client.admit_runtime_workspace("Query Playbook").await?;
            let frame = client
                .dispatch_playbook_method(
                    agent_semantic_client_protocol::WORKSPACE_QUERY_PLAYBOOK_METHOD.to_owned(),
                    serde_json::to_value(
                        agent_semantic_client_protocol::AspClientWorkspaceQueryPlaybookRequest {
                            schema_id: "agent.semantic-protocols.asp-client-workspace-query-playbook-request".to_owned(),
                            schema_version: "1".to_owned(),
                            language,
                            documents,
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

async fn resolve_registered_workspace(
    workspace_id: Option<&str>,
    state_home: &std::path::Path,
) -> Result<PathBuf, String> {
    let current_directory = std::env::current_dir()
        .map_err(|error| format!("failed to resolve current project directory: {error}"))?;
    let current =
        agent_semantic_client_core::state_core::ResolvedState::resolve(current_directory)?;
    let Some(workspace_id) = workspace_id else {
        return Ok(current.workspace.root);
    };
    let catalog_path = agent_semantic_artifacts::StateHomeLayout::new(state_home)
        .runtime_state()
        .serving()
        .workspace_admission_catalog();
    let catalog = agent_semantic_client_db::runtime_server_admission_catalog::
        RuntimeWorkspaceAdmissionCatalog::load(catalog_path)
        .await?;
    let entry =
        catalog.resolve_project_workspace(&current.repo.repo_id.to_string(), workspace_id)?;
    let resolved =
        agent_semantic_client_core::state_core::ResolvedState::resolve(&entry.project_root)?;
    if resolved.repo.repo_id != current.repo.repo_id
        || resolved.workspace.workspace_id.as_str() != workspace_id
    {
        return Err(format!(
            "registered workspace binding drift: projectId={} workspaceId={workspace_id} projectRoot={}",
            current.repo.repo_id,
            entry.project_root.display()
        ));
    }
    Ok(resolved.workspace.root)
}

#[cfg(test)]
#[path = "../../tests/unit/root_language_facade.rs"]
mod tests;
