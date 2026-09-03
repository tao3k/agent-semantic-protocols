use std::sync::Arc;

use agent_semantic_client_protocol::{ClientProjectId, ClientWorkspaceIdentity};

use super::{
    RuntimeProjectWorkspaceKey, RuntimeQueryGenerationState, wait_for_runtime_query_generation,
    wait_for_runtime_query_generation_change,
};

fn key() -> RuntimeProjectWorkspaceKey {
    RuntimeProjectWorkspaceKey::new(
        ClientProjectId::new("project-first-search").expect("project id"),
        ClientWorkspaceIdentity::new("workspace-first-search").expect("workspace id"),
    )
}

#[tokio::test]
async fn absent_first_search_waits_only_for_the_new_byte_generation_terminal() {
    let key = key();
    let (sender, mut receiver) =
        tokio::sync::watch::channel(Arc::new(std::collections::HashMap::<
            RuntimeProjectWorkspaceKey,
            RuntimeQueryGenerationState,
        >::new()));
    let next_key = key.clone();
    tokio::spawn(async move {
        tokio::task::yield_now().await;
        sender.send_replace(Arc::new(std::collections::HashMap::from([(
            next_key,
            RuntimeQueryGenerationState::Failed {
                expected_generation_digest: Arc::from("blake3-256:byte-generation"),
                reason: Arc::from("fixture terminal"),
            },
        )])));
    });
    let observed =
        wait_for_runtime_query_generation(&mut receiver, &key, std::time::Duration::from_secs(1))
            .await
            .expect("new byte-generation terminal");
    assert!(matches!(
        observed,
        RuntimeQueryGenerationState::Failed { .. }
    ));
}

#[tokio::test]
async fn failed_first_search_does_not_reuse_the_stale_failure_terminal() {
    let key = key();
    let stale = RuntimeQueryGenerationState::Failed {
        expected_generation_digest: Arc::from("blake3-256:stale"),
        reason: Arc::from("stale provider failure"),
    };
    let (sender, mut receiver) = tokio::sync::watch::channel(Arc::new(
        std::collections::HashMap::from([(key.clone(), stale)]),
    ));
    let next_key = key.clone();
    tokio::spawn(async move {
        tokio::task::yield_now().await;
        sender.send_replace(Arc::new(std::collections::HashMap::from([(
            next_key,
            RuntimeQueryGenerationState::Failed {
                expected_generation_digest: Arc::from("blake3-256:fresh"),
                reason: Arc::from("fresh byte-generation failure"),
            },
        )])));
    });
    let observed = wait_for_runtime_query_generation_change(
        &mut receiver,
        &key,
        std::time::Duration::from_secs(1),
    )
    .await
    .expect("fresh recovery terminal");
    let RuntimeQueryGenerationState::Failed { reason, .. } = observed else {
        panic!("fixture publishes a replacement failure")
    };
    assert_eq!(reason.as_ref(), "fresh byte-generation failure");
}
