// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use agent_semantic_client_protocol::{
    AspClientWorkspaceSyntaxQueryEvidence, AspClientWorkspaceSyntaxQueryRequest,
    AspClientWorkspaceSyntaxQueryResponse,
};
use agent_semantic_provider_protocol::{
    PROVIDER_SYNTAX_QUERY_OPERATION, PROVIDER_SYNTAX_QUERY_REQUEST_SCHEMA_ID,
    PROVIDER_SYNTAX_QUERY_RESPONSE_SCHEMA_ID, ProviderSyntaxQueryRequest,
    ProviderSyntaxQueryResponse,
};

use crate::RuntimeQueryGeneration;
use crate::runtime_asp_client::AspClientOperationError;

pub(super) async fn dispatch_workspace_syntax_query(
    params: AspClientWorkspaceSyntaxQueryRequest,
    project_root: &Path,
    generation: &RuntimeQueryGeneration,
    providers: &[agent_semantic_search::WorkspaceSearchProvider],
    runtime_search_service: &agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
) -> Result<serde_json::Value, AspClientOperationError> {
    let evidence = execute_workspace_syntax_query_evidence(
        &params,
        project_root,
        generation,
        providers,
        runtime_search_service,
        3,
        None,
    )
    .await?;
    serde_json::to_value(AspClientWorkspaceSyntaxQueryResponse {
        schema_id: "agent.semantic-protocols.asp-client-workspace-syntax-query-response".to_owned(),
        schema_version: "1".to_owned(),
        state: "ready".to_owned(),
        evidence,
    })
    .map_err(|error| AspClientOperationError::Message(error.to_string()))
}

pub(super) async fn execute_workspace_syntax_query_evidence(
    params: &AspClientWorkspaceSyntaxQueryRequest,
    project_root: &Path,
    generation: &RuntimeQueryGeneration,
    providers: &[agent_semantic_search::WorkspaceSearchProvider],
    runtime_search_service: &agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
    limit: usize,
    admitted_owner_paths: Option<&BTreeSet<String>>,
) -> Result<Vec<AspClientWorkspaceSyntaxQueryEvidence>, AspClientOperationError> {
    if limit == 0 || limit > 4096 {
        return Err(AspClientOperationError::Message(
            "workspace syntax Query evidence limit must be in 1..=4096".to_owned(),
        ));
    }
    let selected = params
        .languages
        .iter()
        .chain(params.documents.iter())
        .flat_map(|expression| expression.split('|'))
        .collect::<BTreeSet<_>>();
    let provider_index = providers
        .iter()
        .map(|provider| (provider.language_id.as_str(), provider))
        .collect::<BTreeMap<_, _>>();
    let mut evidence = Vec::new();
    let mut seen_selectors = BTreeSet::new();

    for block in &params.syntax {
        if !selected.contains(block.producer.as_str()) {
            return Err(AspClientOperationError::Message(format!(
                "syntax producer is not selected by --languages/--documents: {}",
                block.producer
            )));
        }
        let provider = provider_index.get(block.producer.as_str()).ok_or_else(|| {
            AspClientOperationError::Message(format!(
                "workspace syntax Query provider is not registered: {}",
                block.producer
            ))
        })?;
        query_provider_block(
            &block.argv,
            project_root,
            generation,
            provider,
            runtime_search_service,
            &mut evidence,
            &mut seen_selectors,
            limit,
            admitted_owner_paths,
        )
        .await?;
    }
    Ok(evidence)
}

async fn query_provider_block(
    argv: &[String],
    project_root: &Path,
    generation: &RuntimeQueryGeneration,
    provider: &agent_semantic_search::WorkspaceSearchProvider,
    runtime_search_service: &agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
    evidence: &mut Vec<AspClientWorkspaceSyntaxQueryEvidence>,
    seen_selectors: &mut BTreeSet<String>,
    limit: usize,
    admitted_owner_paths: Option<&BTreeSet<String>>,
) -> Result<(), AspClientOperationError> {
    let query_source = native_tree_sitter_query(argv)?;
    let plan =
        agent_semantic_client_core::compile_query_abi_source(query_source).map_err(|error| {
            AspClientOperationError::Message(format!(
                "invalid provider-native syntax query for {}: {}",
                provider.language_id, error.message
            ))
        })?;
    let query_digest = blake3::hash(
        &serde_json::to_vec(argv)
            .map_err(|error| AspClientOperationError::Message(error.to_string()))?,
    )
    .to_hex()
    .to_string();
    let source_extensions = provider
        .source_extensions
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    if admitted_owner_paths.is_some_and(BTreeSet::is_empty) {
        return Ok(());
    }

    runtime_search_service
        .provider_runtime(project_root.to_path_buf(), provider.language_id.clone())
        .await
        .map_err(AspClientOperationError::Message)?;
    runtime_search_service
        .provider_runtime_await_ready(
            project_root.to_path_buf(),
            provider.language_id.clone(),
            agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation::new(
            ),
        )
        .await
        .map_err(AspClientOperationError::Message)?;

    for owner_path in generation.resident().indexed_owner_paths() {
        if evidence.len() == limit {
            break;
        }
        let extension = Path::new(&owner_path)
            .extension()
            .and_then(|value| value.to_str());
        if !extension.is_some_and(|value| source_extensions.contains(value)) {
            continue;
        }
        if admitted_owner_paths.is_some_and(|owners| !owners.contains(&owner_path)) {
            continue;
        }
        let provider_request = provider_request_for_owner(
            generation,
            provider,
            &owner_path,
            &query_digest,
            plan.clone(),
        )?;
        let response = execute_provider_query(
            project_root,
            provider,
            runtime_search_service,
            &provider_request,
        )
        .await?;
        append_captures(
            provider,
            &owner_path,
            response,
            evidence,
            seen_selectors,
            limit,
        )?;
    }
    Ok(())
}

fn provider_request_for_owner(
    generation: &RuntimeQueryGeneration,
    provider: &agent_semantic_search::WorkspaceSearchProvider,
    owner_path: &str,
    query_digest: &str,
    plan: agent_semantic_provider_protocol::SyntaxQueryPlan,
) -> Result<ProviderSyntaxQueryRequest, AspClientOperationError> {
    let owner = generation
        .resident()
        .owner_snapshot(owner_path)
        .map_err(AspClientOperationError::Message)?
        .ok_or_else(|| {
            AspClientOperationError::Message(format!(
                "resident syntax Query owner disappeared: {owner_path}"
            ))
        })?;
    let source = String::from_utf8(owner.bytes).map_err(|error| {
        AspClientOperationError::Message(format!(
            "resident syntax Query owner is not UTF-8: owner={owner_path} error={error}"
        ))
    })?;
    let source_content_digest = blake3::hash(source.as_bytes()).to_hex().to_string();
    if !owner.content_digest.ends_with(&source_content_digest) {
        return Err(AspClientOperationError::Message(format!(
            "resident syntax Query owner digest drift: {owner_path}"
        )));
    }
    Ok(ProviderSyntaxQueryRequest {
        schema_id: PROVIDER_SYNTAX_QUERY_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
        owner_path: owner_path.to_owned(),
        source_content_digest,
        query_digest: query_digest.to_owned(),
        source,
        plan,
    })
}

async fn execute_provider_query(
    project_root: &Path,
    provider: &agent_semantic_search::WorkspaceSearchProvider,
    runtime_search_service: &agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
    provider_request: &ProviderSyntaxQueryRequest,
) -> Result<ProviderSyntaxQueryResponse, AspClientOperationError> {
    let payload = serde_json::to_vec(provider_request)
        .map_err(|error| AspClientOperationError::Message(error.to_string()))?;
    let cancellation =
        agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation::new();
    let response = runtime_search_service
        .provider_operation(
            project_root.to_path_buf(),
            provider.language_id.clone(),
            PROVIDER_SYNTAX_QUERY_OPERATION.to_owned(),
            payload,
            cancellation,
        )
        .await
        .map_err(AspClientOperationError::Message)?;
    let response: ProviderSyntaxQueryResponse =
        serde_json::from_slice(&response).map_err(|error| {
            AspClientOperationError::Message(format!(
                "decode provider syntax Query response: {error}"
            ))
        })?;
    validate_provider_response(provider_request, &response)
        .map_err(AspClientOperationError::Message)?;
    Ok(response)
}

fn append_captures(
    provider: &agent_semantic_search::WorkspaceSearchProvider,
    owner_path: &str,
    response: ProviderSyntaxQueryResponse,
    evidence: &mut Vec<AspClientWorkspaceSyntaxQueryEvidence>,
    seen_selectors: &mut BTreeSet<String>,
    limit: usize,
) -> Result<(), AspClientOperationError> {
    let selector_prefix = format!("{}://{}#item/", provider.language_id, owner_path);
    for capture in response.captures {
        if evidence.len() == limit {
            break;
        }
        if !capture.structural_selector.starts_with(&selector_prefix) {
            return Err(AspClientOperationError::Message(format!(
                "provider syntax Query returned a non-canonical selector: {}",
                capture.structural_selector
            )));
        }
        if seen_selectors.insert(capture.structural_selector.clone()) {
            evidence.push(AspClientWorkspaceSyntaxQueryEvidence {
                owner: owner_path.to_owned(),
                selector: capture.structural_selector,
                relation: format!("syntax-capture:{}", capture.capture_name),
            });
        }
    }
    Ok(())
}

fn native_tree_sitter_query(argv: &[String]) -> Result<&str, AspClientOperationError> {
    for (index, argument) in argv.iter().enumerate() {
        if argument == "--treesitter-query" {
            return argv.get(index + 1).map(String::as_str).ok_or_else(|| {
                AspClientOperationError::Message(
                    "provider-native --treesitter-query requires an S-expression".to_owned(),
                )
            });
        }
        if let Some(query) = argument.strip_prefix("--treesitter-query=") {
            return Ok(query);
        }
    }
    Err(AspClientOperationError::Message(
        "registered provider syntax contract requires --treesitter-query <s-expr>".to_owned(),
    ))
}

fn validate_provider_response(
    request: &ProviderSyntaxQueryRequest,
    response: &ProviderSyntaxQueryResponse,
) -> Result<(), String> {
    if response.schema_id != PROVIDER_SYNTAX_QUERY_RESPONSE_SCHEMA_ID
        || response.schema_version != "1"
        || response.language_id != request.language_id
        || response.provider_id != request.provider_id
        || response.owner_path != request.owner_path
        || response.source_content_digest != request.source_content_digest
        || response.query_digest != request.query_digest
        || !response.parsed
    {
        return Err("provider syntax Query response identity drift".to_owned());
    }
    if response.captures.iter().any(|capture| {
        capture.capture_name.trim().is_empty()
            || capture.native_fact_ref.trim().is_empty()
            || capture.structural_selector.trim().is_empty()
            || capture.source_byte_start >= capture.source_byte_end
            || capture.source_byte_end > request.source.len() as u64
    }) {
        return Err("provider syntax Query response contains an invalid capture".to_owned());
    }
    Ok(())
}
