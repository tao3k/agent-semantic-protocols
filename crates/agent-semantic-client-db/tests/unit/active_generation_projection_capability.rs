//! Tokio concurrency gates for active-generation projection capability publication.

use std::collections::BTreeSet;

use agent_semantic_client_db::active_generation_projection_capability::{
    ACTIVE_GENERATION_PROJECTION_CAPABILITY_SCHEMA_ID,
    ACTIVE_GENERATION_PROJECTION_CAPABILITY_SCHEMA_VERSION, ActiveGenerationCapabilityState,
    ActiveGenerationProjectionCapabilityPublisher, ActiveGenerationProjectionCapabilityReader,
    ActiveGenerationProjectionCapabilityReceipt, ActiveGenerationProjectionMode,
    ActiveGenerationSelectorCapability, active_generation_projection_capability_channel,
};

fn ready_receipt(epoch: u64) -> ActiveGenerationProjectionCapabilityReceipt {
    ActiveGenerationProjectionCapabilityReceipt {
        schema_id: ACTIVE_GENERATION_PROJECTION_CAPABILITY_SCHEMA_ID.to_owned(),
        schema_version: ACTIVE_GENERATION_PROJECTION_CAPABILITY_SCHEMA_VERSION.to_owned(),
        state: ActiveGenerationCapabilityState::Ready,
        workspace_identity: "workspace-capability-test".to_owned(),
        generation_digest: format!("blake3-256:{:064x}", epoch),
        root_digest: format!("blake3-256:{:064x}", epoch + 1),
        provider_catalog_digest: format!("sha256:{:064x}", epoch + 2),
        provider_catalog_readable: true,
        publication_epoch: epoch,
        selectors: vec![ActiveGenerationSelectorCapability {
            selector: "rust://src/lib.rs#item/function/example".to_owned(),
            owner_path: "src/lib.rs".to_owned(),
            projection_modes: BTreeSet::from([
                ActiveGenerationProjectionMode::Source,
                ActiveGenerationProjectionMode::CallableSkeleton,
            ]),
        }],
        failure: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_readers_observe_one_atomic_projection_capability_epoch() {
    let (publisher, reader) = active_generation_projection_capability_channel();
    let mut tasks = Vec::with_capacity(256);
    for _ in 0..256 {
        let mut reader = reader.clone();
        tasks.push(tokio::spawn(async move { reader.changed().await }));
    }

    tokio::task::yield_now().await;
    publisher.publish(ready_receipt(1)).expect("publish epoch");

    for task in tasks {
        let receipt = task.await.expect("reader task").expect("published receipt");
        assert_eq!(receipt.publication_epoch, 1);
        assert!(receipt.admits(
            "rust://src/lib.rs#item/function/example",
            ActiveGenerationProjectionMode::Source,
        ));
    }
}

#[tokio::test]
async fn publication_epoch_is_monotonic_and_failed_publish_is_not_visible() {
    let (publisher, reader) = active_generation_projection_capability_channel();
    publisher
        .publish(ready_receipt(2))
        .expect("publish epoch 2");

    let error = publisher
        .publish(ready_receipt(1))
        .expect_err("reject stale epoch");
    assert!(error.contains("publication epoch is not monotonic"));
    assert_eq!(
        reader.current().expect("current receipt").publication_epoch,
        2
    );
}

#[tokio::test]
async fn closing_the_tokio_authority_releases_waiting_readers() {
    let (publisher, mut reader): (
        ActiveGenerationProjectionCapabilityPublisher,
        ActiveGenerationProjectionCapabilityReader,
    ) = active_generation_projection_capability_channel();
    drop(publisher);

    let error = reader.changed().await.expect_err("publisher is closed");
    assert!(error.contains("publisher closed"));
}
