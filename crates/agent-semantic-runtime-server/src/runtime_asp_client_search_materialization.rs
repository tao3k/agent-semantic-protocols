// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Awaits the single-flight Search materialization terminal for one generation.

use super::{AspClientDispatchError, AspClientOperationError};

pub(super) fn search_materialization_dispatch_error(
    error: AspClientOperationError,
) -> AspClientDispatchError {
    match error {
        AspClientOperationError::Terminal(error) => error,
        AspClientOperationError::Message(message) => AspClientDispatchError {
            reason_kind: "search-materialization-failed".to_owned(),
            message,
            details: Some(serde_json::json!({
                "schemaId": "agent.semantic-protocols.asp-client-dispatch-failure",
                "schemaVersion": "1",
                "state": "failed",
                "phase": "runtime-search-materialization",
                "reasonKind": "search-materialization-failed"
            })),
        },
    }
}

pub(super) async fn settled_search_materialization(
    generation: &crate::runtime_query_generation::RuntimeQueryGeneration,
    key: &str,
) -> Result<agent_semantic_client_protocol::ClientResponsePayload, AspClientOperationError> {
    use crate::runtime_query_generation::RuntimeSearchMaterializationState;
    match generation.await_search_materialization(key).await? {
        RuntimeSearchMaterializationState::Ready(result) => {
            Ok(agent_semantic_client_protocol::ClientResponsePayload::from_shared(result))
        }
        RuntimeSearchMaterializationState::Failed(error) => {
            Err(AspClientOperationError::Terminal(error.as_ref().clone()))
        }
        RuntimeSearchMaterializationState::Building(_) => Err(AspClientOperationError::Message(
            "Search completion preceded terminal publication".to_owned(),
        )),
    }
}
