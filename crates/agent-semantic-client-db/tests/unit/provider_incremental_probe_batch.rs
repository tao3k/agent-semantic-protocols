// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::ProviderOwnerBatchProbeRequest;
use agent_semantic_client_db::ProviderOwnerDecision;
use agent_semantic_client_db::ProviderOwnerInventoryState;
use agent_semantic_client_db::ProviderOwnerInventoryWrite;
use agent_semantic_client_db::ProviderOwnerMetadata;
use agent_semantic_client_db::WorkspaceDbRegistry;

use crate::test_support::StateHomeGuard;
use crate::test_support::TestDir;
use crate::test_support::environment_lock;
use crate::test_support::workspace;

fn requests(count: usize) -> Vec<ProviderOwnerBatchProbeRequest> {
    (0..count)
        .map(|index| ProviderOwnerBatchProbeRequest {
            owner_path: format!("src/owner-{index:04}.rs"),
            metadata: ProviderOwnerMetadata {
                file_identity: format!("file-{index}"),
                size_bytes: 10,
                modified_unix_nanos: 20,
                change_time_unix_nanos: 30,
            },
        })
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn resident_session_classifies_thousand_owners_without_reopening_state() {
    let _environment = environment_lock();
    let temp = TestDir::new("batch-probe-resident");
    let _state_home = StateHomeGuard::install(&temp.path().join("state"));
    let (project_root, _resolved, mut scope) =
        workspace(temp.path(), "batch-probe-resident-workspace");
    scope.provider_workspace_identity_digest = format!("{:064x}", 7);
    let registry = WorkspaceDbRegistry::default();
    let session = registry
        .acquire(&project_root, &scope)
        .await
        .expect("acquire resident workspace session");
    let receipt = session
        .probe_provider_owners(&scope, requests(1_000).as_slice())
        .await
        .expect("batch probe resident state");

    assert_eq!(receipt.results.len(), 1_000);
    assert_eq!(receipt.read_lock_count, 0);
    assert_eq!(receipt.connection_open_count, 0);
    assert_eq!(receipt.scope_scan_count, 1);
    let counters = registry.counters();
    assert_eq!(counters.database_open_count, 1);
    assert_eq!(counters.connection_create_count, 2);
    assert_eq!(counters.schema_bootstrap_count, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn resident_session_reuses_open_state_for_inventory_write_and_batch_probe() {
    let _environment = environment_lock();
    let temp = TestDir::new("batch-probe-reuse");
    let _state_home = StateHomeGuard::install(&temp.path().join("state"));
    let (project_root, _resolved, mut scope) =
        workspace(temp.path(), "batch-probe-reuse-workspace");
    scope.provider_workspace_identity_digest = format!("{:064x}", 7);
    let registry = WorkspaceDbRegistry::default();
    let session = registry
        .acquire(&project_root, &scope)
        .await
        .expect("acquire resident workspace session");
    session
        .upsert_provider_owner_inventory(&ProviderOwnerInventoryWrite {
            scope: scope.clone(),
            state: ProviderOwnerInventoryState::Exact,
            entries: Vec::new(),
        })
        .await
        .expect("write inventory through resident session");

    let receipt = session
        .probe_provider_owners(&scope, requests(1_000).as_slice())
        .await
        .expect("batch probe");

    assert_eq!(receipt.results.len(), 1_000);
    assert_eq!(receipt.read_lock_count, 0);
    assert_eq!(receipt.connection_open_count, 0);
    assert_eq!(receipt.scope_scan_count, 1);
    assert!(
        receipt
            .results
            .iter()
            .all(|result| result.probe.decision == ProviderOwnerDecision::New)
    );
    let counters = registry.counters();
    assert_eq!(counters.database_open_count, 1);
    assert_eq!(counters.connection_create_count, 2);
    assert_eq!(counters.schema_bootstrap_count, 1);
}
