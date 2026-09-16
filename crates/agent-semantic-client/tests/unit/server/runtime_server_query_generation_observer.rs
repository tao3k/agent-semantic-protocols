// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::publish_observer_terminal;

#[tokio::test]
async fn failed_terminal_publishes_exact_generation_failure() {
    let authority = agent_semantic_runtime_server::RuntimeQueryGenerationAuthority::new();
    let project_id = agent_semantic_client_protocol::ClientProjectId::new("repo-project")
        .expect("project identity");
    let workspace_id = agent_semantic_client_protocol::ClientWorkspaceIdentity::new("workspace")
        .expect("workspace identity");
    let key = agent_semantic_runtime_server::query_generation::RuntimeProjectWorkspaceKey::new(
        project_id.clone(),
        workspace_id.clone(),
    );
    let publication =
        agent_semantic_client_db::runtime_server_publication::WorkspaceGenerationPublished {
            project_id,
            workspace_id,
            project_root: std::path::PathBuf::from("/project"),
            resident_pointer_path: std::path::PathBuf::from("/generation.pointer"),
            generation_digest: "blake3-256:generation".to_owned(),
        };
    let receiver = authority.subscribe();

    publish_observer_terminal(
        &authority,
        key.clone(),
        &publication,
        Err("first root cause".to_owned()),
    );

    let snapshot = receiver.borrow();
    let Some(agent_semantic_runtime_server::RuntimeQueryGenerationState::Failed {
        expected_generation_digest,
        reason,
    }) = snapshot.get(&key)
    else {
        panic!("observer failure must close the exact generation as Failed");
    };
    assert_eq!(
        expected_generation_digest.as_ref(),
        publication.generation_digest
    );
    assert_eq!(reason.as_ref(), "first root cause");
}
