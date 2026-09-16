// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Awaits the single-flight Search materialization terminal for one generation.

use super::{AspClientDispatchError, AspClientOperationError};

pub(super) struct SearchMaterializationPublicationGuard {
    generation: std::sync::Arc<crate::RuntimeQueryGeneration>,
    key: Option<String>,
}

impl SearchMaterializationPublicationGuard {
    pub(super) fn new(
        generation: std::sync::Arc<crate::RuntimeQueryGeneration>,
        key: String,
    ) -> Self {
        Self {
            generation,
            key: Some(key),
        }
    }

    pub(super) fn publish(
        mut self,
        result: Result<serde_json::Value, AspClientDispatchError>,
    ) -> Result<(), String> {
        let key = self
            .key
            .take()
            .expect("Search materialization publication guard must own its key");
        self.generation.publish_search_materialization(key, result)
    }
}

impl Drop for SearchMaterializationPublicationGuard {
    fn drop(&mut self) {
        let Some(key) = self.key.take() else {
            return;
        };
        let terminal = AspClientDispatchError {
            reason_kind: "search-materialization-cancelled".to_owned(),
            message: "Search materialization task ended before terminal publication".to_owned(),
            details: Some(serde_json::json!({
                "schemaId": "agent.semantic-protocols.asp-client-dispatch-failure",
                "schemaVersion": "1",
                "state": "cancelled",
                "phase": "runtime-search-materialization",
                "reasonKind": "search-materialization-cancelled"
            })),
        };
        let _ = self
            .generation
            .publish_search_materialization(key, Err(terminal));
    }
}

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
    use crate::runtime_query_generation::RuntimeSearchTerminalState;
    match generation.await_search_materialization(key).await? {
        RuntimeSearchTerminalState::Ready(result) => {
            Ok(agent_semantic_client_protocol::ClientResponsePayload::from_shared(result))
        }
        RuntimeSearchTerminalState::Failed(error) => {
            Err(AspClientOperationError::Terminal(error.as_ref().clone()))
        }
    }
}
