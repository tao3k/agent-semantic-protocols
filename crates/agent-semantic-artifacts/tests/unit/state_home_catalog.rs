//! State Home catalog tests.

use crate::{
    CatalogObservation, CleanupDisposition, ProjectBinding, RetainedObject, RetentionLease,
    RetentionObjectKind, StateHomeCatalog, StateHomeLayout,
};

#[tokio::test(flavor = "current_thread")]
async fn catalog_transaction_publishes_binding_observation_lease_and_generation_together() {
    let temp = tempfile::tempdir().unwrap();
    let layout = StateHomeLayout::new(temp.path());
    let catalog = StateHomeCatalog::open(&layout.catalog).await.unwrap();
    let binding = ProjectBinding::resolve(None, "repo", temp.path()).unwrap();
    let object = RetainedObject {
        object_id: "workspace:current".to_string(),
        kind: RetentionObjectKind::Workspace,
        last_observed_at_ms: 10,
        byte_count: 42,
    };
    let lease = RetentionLease {
        lease_id: "active-workspace".to_string(),
        object_id: object.object_id.clone(),
        owner: "runtime".to_string(),
        expires_at_ms: None,
    };

    let first = catalog
        .observe(&binding, &object, std::slice::from_ref(&lease), 10)
        .await
        .unwrap();
    let second = catalog
        .observe(&binding, &object, std::slice::from_ref(&lease), 11)
        .await
        .unwrap();
    assert_eq!(first.generation.get(), 1);
    assert_eq!(second.generation.get(), 2);
    assert_eq!(second.lease_count, 1);

    let plan = catalog.plan_cleanup(1_000, 1).await.unwrap();
    assert_eq!(plan.retained_count, 1);
    assert_eq!(plan.retired_count, 0);
    assert!(matches!(
        plan.entries[0].disposition,
        CleanupDisposition::Keep { .. }
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn catalog_replacement_removes_stale_lease_in_the_same_transaction() {
    let temp = tempfile::tempdir().unwrap();
    let layout = StateHomeLayout::new(temp.path());
    let catalog = StateHomeCatalog::open(&layout.catalog).await.unwrap();
    let binding = ProjectBinding::resolve(None, "repo", temp.path()).unwrap();
    let object = RetainedObject {
        object_id: "artifact:old".to_string(),
        kind: RetentionObjectKind::Artifact,
        last_observed_at_ms: 10,
        byte_count: 7,
    };
    let lease = RetentionLease {
        lease_id: "pending".to_string(),
        object_id: object.object_id.clone(),
        owner: "runtime".to_string(),
        expires_at_ms: None,
    };
    catalog
        .observe(&binding, &object, &[lease], 10)
        .await
        .unwrap();
    catalog.observe(&binding, &object, &[], 20).await.unwrap();

    let plan = catalog.plan_cleanup(1_000, 1).await.unwrap();
    assert_eq!(plan.retained_count, 0);
    assert_eq!(plan.retired_count, 1);
    assert!(matches!(
        plan.entries[0].disposition,
        CleanupDisposition::Retire { .. }
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn observation_batch_advances_one_generation_for_many_objects() {
    let temp = tempfile::tempdir().unwrap();
    let layout = StateHomeLayout::new(temp.path());
    let catalog = StateHomeCatalog::open(&layout.catalog).await.unwrap();
    let binding = ProjectBinding::resolve(None, "repo", temp.path()).unwrap();
    let observations = ["artifact:first", "artifact:second"].map(|object_id| CatalogObservation {
        binding: binding.clone(),
        object: RetainedObject {
            object_id: object_id.to_string(),
            kind: RetentionObjectKind::Artifact,
            last_observed_at_ms: 10,
            byte_count: 1,
        },
        leases: Vec::new(),
        observed_at_ms: 10,
    });

    let receipt = catalog.observe_batch(&observations).await.unwrap();
    assert_eq!(receipt.generation.get(), 1);
    assert_eq!(receipt.observation_count, 2);
    assert_eq!(catalog.plan_cleanup(20, 1).await.unwrap().retired_count, 2);
}

#[tokio::test(flavor = "current_thread")]
async fn empty_observation_batch_is_a_generation_preserving_no_op() {
    let temp = tempfile::tempdir().unwrap();
    let layout = StateHomeLayout::new(temp.path());
    let catalog = StateHomeCatalog::open(&layout.catalog).await.unwrap();

    let receipt = catalog.observe_batch(&[]).await.unwrap();

    assert_eq!(receipt.generation.get(), 0);
    assert_eq!(receipt.observation_count, 0);
    assert_eq!(catalog.generation().await.unwrap().get(), 0);
    assert!(
        catalog
            .plan_cleanup(20, 1)
            .await
            .unwrap()
            .entries
            .is_empty()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_batch_does_not_advance_catalog_generation() {
    let temp = tempfile::tempdir().unwrap();
    let layout = StateHomeLayout::new(temp.path());
    let catalog = StateHomeCatalog::open(&layout.catalog).await.unwrap();
    let binding = ProjectBinding::resolve(None, "repo", temp.path()).unwrap();
    let error = catalog
        .observe_batch(&[CatalogObservation {
            binding,
            object: RetainedObject {
                object_id: "artifact:valid".to_string(),
                kind: RetentionObjectKind::Artifact,
                last_observed_at_ms: 10,
                byte_count: 1,
            },
            leases: vec![RetentionLease {
                lease_id: "wrong-object".to_string(),
                object_id: "artifact:different".to_string(),
                owner: "runtime".to_string(),
                expires_at_ms: None,
            }],
            observed_at_ms: 10,
        }])
        .await
        .unwrap_err();
    assert!(error.contains("targets a different object"));
    assert_eq!(catalog.generation().await.unwrap().get(), 0);
    assert!(
        catalog
            .plan_cleanup(20, 1)
            .await
            .unwrap()
            .entries
            .is_empty()
    );
}
