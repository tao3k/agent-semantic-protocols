use agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDurabilityReceipt;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDurabilityState;

#[test]
fn durability_receipt_requires_failure_only_for_failed_state() {
    for state in [
        WorkspaceGenerationDurabilityState::ResidentReady,
        WorkspaceGenerationDurabilityState::DurableReady,
    ] {
        WorkspaceGenerationDurabilityReceipt::new(
            "workspace-a",
            "blake3-256:generation-a",
            1,
            state,
            None,
        )
        .expect("successful durability receipt");
        assert!(
            WorkspaceGenerationDurabilityReceipt::new(
                "workspace-a",
                "blake3-256:generation-a",
                1,
                state,
                Some("unexpected".to_owned()),
            )
            .is_err()
        );
    }

    let failed = WorkspaceGenerationDurabilityReceipt::new(
        "workspace-a",
        "blake3-256:generation-a",
        1,
        WorkspaceGenerationDurabilityState::Failed,
        Some("sync failed".to_owned()),
    )
    .expect("failed durability receipt carries cause");
    assert_eq!(failed.state, WorkspaceGenerationDurabilityState::Failed);
    assert!(
        WorkspaceGenerationDurabilityReceipt::new(
            "workspace-a",
            "blake3-256:generation-a",
            1,
            WorkspaceGenerationDurabilityState::Failed,
            None,
        )
        .is_err()
    );
}
