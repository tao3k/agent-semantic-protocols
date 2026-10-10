// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Constant-time resident context read for Client-side enhanced Query lowering.

use agent_semantic_client_protocol::{
    AspClientWorkspaceSyntaxPlanContextRequest, AspClientWorkspaceSyntaxPlanContextResponse,
};

use crate::RuntimeQueryGeneration;
use crate::runtime_asp_client::AspClientOperationError;

pub(super) fn dispatch_workspace_syntax_plan_context(
    params: AspClientWorkspaceSyntaxPlanContextRequest,
    generation: &RuntimeQueryGeneration,
    providers: &[agent_semantic_search::WorkspaceSearchProvider],
) -> Result<serde_json::Value, AspClientOperationError> {
    let response =
        syntax_plan_context(&params.producer, generation.generation_digest(), providers)?;
    serde_json::to_value(response)
        .map_err(|error| AspClientOperationError::Message(error.to_string()))
}

fn syntax_plan_context(
    producer: &str,
    generation_digest: &str,
    providers: &[agent_semantic_search::WorkspaceSearchProvider],
) -> Result<AspClientWorkspaceSyntaxPlanContextResponse, AspClientOperationError> {
    let provider = providers
        .iter()
        .find(|provider| provider.language_id == producer)
        .ok_or_else(|| {
            AspClientOperationError::Message(format!(
                "workspace syntax plan context provider is not registered: {producer}"
            ))
        })?;
    let capability = provider.enhanced_query_capability.as_ref().ok_or_else(|| {
        AspClientOperationError::Message(format!(
            "enhanced-query-capability-table-missing producer={producer}"
        ))
    })?;
    capability
        .validate()
        .map_err(AspClientOperationError::Message)?;
    Ok(AspClientWorkspaceSyntaxPlanContextResponse {
        schema_id: "agent.semantic-protocols.asp-client-workspace-syntax-plan-context-response"
            .to_owned(),
        schema_version: "1".to_owned(),
        generation_digest: generation_digest.to_owned(),
        capability: capability.clone(),
    })
}

#[cfg(test)]
#[path = "../tests/unit/runtime_asp_client_syntax_plan_context.rs"]
mod tests;
