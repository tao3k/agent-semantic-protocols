use super::StateHomeCatalog;
use agent_semantic_artifacts::{
    CatalogGeneration, CleanupSelection, ProjectBinding, RetainedObject, RetentionObjectKind,
};

#[tokio::test]
async fn cleanup_selection_targets_one_workspace_and_missing_identity_fails_closed() {
    let temporary = tempfile::tempdir().expect("catalog selection fixture");
    let first_root = temporary.path().join("first");
    let second_root = temporary.path().join("second");
    std::fs::create_dir_all(&first_root).expect("first workspace root");
    std::fs::create_dir_all(&second_root).expect("second workspace root");
    let catalog = StateHomeCatalog::open(temporary.path().join("catalog/state.turso"))
        .await
        .expect("open catalog");
    let first =
        ProjectBinding::resolve(None, "git-common-dir:first", &first_root).expect("first binding");
    let second = ProjectBinding::resolve(None, "git-common-dir:second", &second_root)
        .expect("second binding");
    for (binding, object_id) in [(&first, "cache:first"), (&second, "cache:second")] {
        catalog
            .observe(
                binding,
                &RetainedObject {
                    object_id: object_id.to_string(),
                    kind: RetentionObjectKind::Workspace,
                    last_observed_at_ms: 1,
                    byte_count: 10,
                },
                &[],
                1,
            )
            .await
            .expect("observe retained workspace");
    }

    let plan = catalog
        .plan_cleanup_selected(
            100,
            10,
            CleanupSelection::WorkspaceDigest {
                workspace_digest: first.workspace.digest.to_string(),
            },
        )
        .await
        .expect("plan exact workspace cleanup");
    assert_eq!(plan.entries.len(), 1);
    assert_eq!(plan.entries[0].object.object_id, "cache:first");

    let error = catalog
        .plan_cleanup_selected(
            100,
            10,
            CleanupSelection::ObjectId {
                object_id: "cache:missing".to_string(),
            },
        )
        .await
        .expect_err("missing exact cleanup object must fail closed");
    assert!(error.contains("state-home-cleanup-selection-no-match"));
}

#[tokio::test]
async fn deletion_generation_mismatch_preserves_the_catalog_object() {
    let temporary = tempfile::tempdir().expect("catalog deletion fixture");
    let workspace_root = temporary.path().join("workspace");
    std::fs::create_dir_all(&workspace_root).expect("workspace root");
    let catalog = StateHomeCatalog::open(temporary.path().join("catalog/state.turso"))
        .await
        .expect("open catalog");
    let binding =
        ProjectBinding::resolve(None, "git-common-dir:repo", &workspace_root).expect("binding");
    let object = RetainedObject {
        object_id: "workspace:delete".to_string(),
        kind: RetentionObjectKind::Workspace,
        last_observed_at_ms: 1,
        byte_count: 10,
    };
    let observed = catalog
        .observe(&binding, &object, &[], 1)
        .await
        .expect("observe workspace");
    let selected = std::collections::BTreeSet::from([object.object_id.clone()]);

    let error = catalog
        .delete_objects(
            CatalogGeneration::new(observed.generation.get() - 1),
            &selected,
        )
        .await
        .expect_err("stale cleanup generation must fail closed");
    assert!(error.contains("state-home-cleanup-catalog-generation-changed"));

    let plan = catalog
        .plan_cleanup_selected(
            100,
            10,
            CleanupSelection::ObjectId {
                object_id: object.object_id.clone(),
            },
        )
        .await
        .expect("stale CAS must preserve the object");
    assert_eq!(plan.entries.len(), 1);

    let committed = catalog
        .delete_objects(observed.generation, &selected)
        .await
        .expect("delete current object");
    assert_eq!(committed.get(), observed.generation.get() + 1);
    let error = catalog
        .plan_cleanup_selected(
            100,
            10,
            CleanupSelection::ObjectId {
                object_id: object.object_id,
            },
        )
        .await
        .expect_err("committed deletion must remove the object");
    assert!(error.contains("state-home-cleanup-selection-no-match"));
}
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
