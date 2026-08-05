use super::{
    RuntimeOwnerIdentityEntry, RuntimeOwnerIdentityJournalPublisher, RuntimeOwnerIdentityState,
};

const BASE_A: &str = "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OWNER_A: &str = "blake3-256:1111111111111111111111111111111111111111111111111111111111111111";
const OWNER_B: &str = "blake3-256:2222222222222222222222222222222222222222222222222222222222222222";

fn present(owner_path: &str, digest: &str) -> RuntimeOwnerIdentityEntry {
    RuntimeOwnerIdentityEntry {
        owner_path: owner_path.to_owned(),
        state: RuntimeOwnerIdentityState::Present,
        content_digest: Some(digest.to_owned()),
        mutation_id: None,
    }
}

#[tokio::test]
async fn mutation_replay_is_idempotent_and_payload_conflict_is_rejected() {
    let temporary = tempfile::tempdir().expect("create owner journal fixture");
    let publisher = RuntimeOwnerIdentityJournalPublisher::open(temporary.path().to_path_buf())
        .await
        .expect("open owner journal publisher");
    publisher
        .publish_delta(
            "workspace-a",
            BASE_A,
            "mutation-1",
            vec![present("src/a.rs", OWNER_A), present("src/b.rs", OWNER_B)],
        )
        .await
        .expect("publish mutation");
    let committed_epoch = publisher
        .state
        .lock()
        .await
        .as_ref()
        .expect("committed snapshot")
        .epoch;
    publisher
        .publish_delta(
            "workspace-a",
            BASE_A,
            "mutation-1",
            vec![present("src/a.rs", OWNER_A), present("src/b.rs", OWNER_B)],
        )
        .await
        .expect("replay identical mutation");
    assert_eq!(
        publisher
            .state
            .lock()
            .await
            .as_ref()
            .expect("replayed snapshot")
            .epoch,
        committed_epoch
    );
    assert!(
        publisher
            .publish_delta(
                "workspace-a",
                BASE_A,
                "mutation-1",
                vec![present("src/a.rs", OWNER_B)],
            )
            .await
            .is_err()
    );
    assert!(
        publisher
            .publish_delta(
                "workspace-a",
                BASE_A,
                "mutation-1",
                vec![present("src/a.rs", OWNER_A)],
            )
            .await
            .is_err()
    );
}
