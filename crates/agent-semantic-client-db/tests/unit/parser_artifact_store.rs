// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{ParserArtifactIdentity, ParserArtifactStore};

fn digest(byte: char) -> String {
    format!(
        "blake3-256:{}",
        std::iter::repeat_n(byte, 64).collect::<String>()
    )
}

fn identity(content: char) -> ParserArtifactIdentity {
    ParserArtifactIdentity {
        provider_id: "asp-rust".to_owned(),
        parser_identity_digest: digest('1'),
        query_pack_digest: digest('2'),
        auxiliary_input_digest: digest('3'),
        owner_path: "src/lib.rs".to_owned(),
        owner_content_digest: digest(content),
    }
}

fn owner(
    content: char,
) -> agent_semantic_provider_transport::projection_batch::ProviderProjectedOwner {
    agent_semantic_provider_transport::projection_batch::ProviderProjectedOwner {
        owner_path: "src/lib.rs".to_owned(),
        source_leaf_digest: digest(content),
        projection_state:
            agent_semantic_provider_transport::projection_batch::ProviderProjectionState::Ready,
        diagnostic: None,
        items: Vec::new(),
        relations: Vec::new(),
    }
}

#[tokio::test]
async fn artifact_reuses_across_generation_proofs_but_not_content_identity() {
    let temporary = tempfile::tempdir().expect("temporary parser artifact root");
    let db_path = temporary.path().join("client.db");
    let store = ParserArtifactStore::for_client_db(&db_path).expect("parser artifact store");
    let stable = identity('a');
    store
        .publish(stable.clone(), owner('a'))
        .await
        .expect("publish parser artifact");

    assert_eq!(
        store
            .read(&stable)
            .await
            .expect("read parser artifact")
            .expect("stable parser artifact"),
        owner('a')
    );
    assert!(
        store
            .read(&identity('b'))
            .await
            .expect("changed content lookup")
            .is_none(),
        "generation-independent reuse must still bind exact owner content"
    );
}

#[tokio::test]
async fn corrupted_artifact_is_rejected_before_reuse() {
    let temporary = tempfile::tempdir().expect("temporary parser artifact root");
    let db_path = temporary.path().join("client.db");
    let store = ParserArtifactStore::for_client_db(&db_path).expect("parser artifact store");
    let stable = identity('a');
    store
        .publish(stable.clone(), owner('a'))
        .await
        .expect("publish parser artifact");
    let artifact_path = store
        .path_for_digest(&stable.key_digest().expect("artifact key"))
        .expect("artifact path");
    std::fs::write(&artifact_path, b"{\"corrupt\":true}").expect("corrupt parser artifact");

    assert!(store.read(&stable).await.is_err());
}
