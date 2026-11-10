// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Closes the daemon-owned resident query-generation observer terminal.

pub(super) fn publish_observer_terminal(
    authority: &agent_semantic_runtime_server::RuntimeQueryGenerationAuthority,
    key: agent_semantic_runtime_server::query_generation::RuntimeProjectWorkspaceKey,
    publication: &agent_semantic_client_db::runtime_server_publication::WorkspaceGenerationPublished,
    result: Result<
        std::sync::Arc<agent_semantic_runtime_server::query_generation::RuntimeQueryGeneration>,
        String,
    >,
) {
    match result {
        Ok(generation) => eprintln!(
            "[runtime-query-generation-observer-terminal] {}",
            serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-query-generation-observer-terminal",
                "schemaVersion": "1",
                "state": "ready",
                "projectId": publication.project_id,
                "workspaceId": publication.workspace_id,
                "generationDigest": generation.generation_digest(),
                "generationToken": generation.generation_token(),
            })
        ),
        Err(error) => {
            authority.publish_failed(key, 0, publication.generation_digest.clone(), error.clone());
            eprintln!(
                "[runtime-query-generation-observer-terminal] {}",
                serde_json::json!({
                    "schemaId": "agent.semantic-protocols.runtime-query-generation-observer-terminal",
                    "schemaVersion": "1",
                    "state": "failed",
                    "reasonKind": "runtime-query-generation-observer-failed",
                    "projectId": publication.project_id,
                    "workspaceId": publication.workspace_id,
                    "generationDigest": publication.generation_digest,
                    "error": error,
                })
            );
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/server/runtime_server_query_generation_observer.rs"]
mod tests;
