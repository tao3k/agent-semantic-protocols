use std::sync::Arc;

use agent_semantic_client_db::{
    ProviderIncrementalOwnerWrite, ProviderOwnerFingerprint, ProviderOwnerMetadata,
    WorkspaceDbRegistry, WorkspaceDbRegistryCounters,
};

use crate::test_support::{StateHomeGuard, environment_lock, workspace};
use tempfile::TempDir;

#[tokio::test(flavor = "current_thread")]
async fn wrong_workspace_identity_fails_before_database_open() {
    let _environment = environment_lock();
    let temp = TempDir::new().expect("create wrong-identity tempfile");
    let state_home = temp.path().join("state");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root, resolved, mut scope) = workspace(temp.path(), "wrong-identity");
    let client_db_path = resolved.paths.client_db_path;
    scope.workspace_identity = "workspace-wrong".to_owned();
    let registry = WorkspaceDbRegistry::default();

    let error = registry
        .acquire(&project_root, &scope)
        .await
        .expect_err("wrong workspace identity must fail closed");

    assert!(error.contains("workspace identity mismatch"));
    assert_eq!(registry.counters(), WorkspaceDbRegistryCounters::default());
    assert!(!client_db_path.exists());
}

#[tokio::test(flavor = "current_thread")]
async fn member_project_root_reuses_the_canonical_workspace_entry() {
    let _environment = environment_lock();
    let temp = TempDir::new().expect("create member-root tempfile");
    let state_home = temp.path().join("state");
    let _state_home = StateHomeGuard::install(&state_home);
    let (workspace_root, _resolved, mut scope) = workspace(temp.path(), "member-root");
    let member_root = workspace_root.join("crates/member");
    std::fs::create_dir_all(&member_root).expect("create nested workspace member");
    scope.project_root = member_root.display().to_string();
    let registry = WorkspaceDbRegistry::default();

    let root_session = registry
        .acquire(&workspace_root, &scope)
        .await
        .expect("workspace root must accept a member-scoped provider request");
    let member_session = registry
        .acquire(&member_root, &scope)
        .await
        .expect("workspace member must resolve through the canonical workspace entry");

    assert_eq!(
        root_session.workspace_identity(),
        member_session.workspace_identity()
    );
    assert_eq!(
        root_session.client_db_path(),
        member_session.client_db_path()
    );
    let counters = registry.counters();
    assert_eq!(counters.database_open_count, 1);
    assert_eq!(counters.connection_create_count, 2);
    assert_eq!(counters.schema_bootstrap_count, 1);
    assert_eq!(counters.registry_hit_count, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn one_hundred_concurrent_leases_open_and_bootstrap_once() {
    let _environment = environment_lock();
    let temp = TempDir::new().expect("create concurrent tempfile");
    let state_home = temp.path().join("state");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root, _resolved, scope) = workspace(temp.path(), "concurrent");
    let registry = Arc::new(WorkspaceDbRegistry::default());
    let mut leases = tokio::task::JoinSet::new();
    for _ in 0..100 {
        let registry = Arc::clone(&registry);
        let project_root = project_root.clone();
        let scope = scope.clone();
        leases.spawn(async move { registry.acquire(project_root, &scope).await });
    }
    let mut sessions = Vec::new();
    while let Some(session) = leases.join_next().await {
        sessions.push(session.expect("concurrent lease task must join"));
    }

    assert_eq!(sessions.len(), 100);
    assert!(sessions.into_iter().all(|session| {
        session
            .expect("concurrent lease must succeed")
            .workspace_identity()
            == scope.workspace_identity
    }));
    let counters = registry.counters();
    assert_eq!(counters.database_open_count, 1);
    assert_eq!(counters.connection_create_count, 2);
    assert_eq!(counters.schema_bootstrap_count, 1);
    assert_eq!(counters.registry_hit_count, 99);
    assert_eq!(counters.workspace_lock_retry_count, 0);
}

#[tokio::test(flavor = "current_thread")]
async fn different_workspaces_initialize_independent_entries() {
    let _environment = environment_lock();
    let temp = TempDir::new().expect("create independent tempfile");
    let state_home = temp.path().join("state");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root_a, _resolved_a, scope_a) = workspace(temp.path(), "workspace-a");
    let (project_root_b, _resolved_b, scope_b) = workspace(temp.path(), "workspace-b");
    let registry = WorkspaceDbRegistry::default();

    let (left, right) = tokio::join!(
        registry.acquire(&project_root_a, &scope_a),
        registry.acquire(&project_root_b, &scope_b),
    );
    let left = left.expect("first workspace lease must succeed");
    let right = right.expect("second workspace lease must succeed");

    assert_ne!(left.workspace_identity(), right.workspace_identity());
    assert_ne!(left.client_db_path(), right.client_db_path());
    let counters = registry.counters();
    assert_eq!(counters.database_open_count, 2);
    assert_eq!(counters.connection_create_count, 4);
    assert_eq!(counters.schema_bootstrap_count, 2);
    assert_eq!(counters.workspace_lock_retry_count, 0);
}

#[tokio::test(flavor = "current_thread")]
async fn one_hundred_concurrent_writes_share_one_serial_writer() {
    let _environment = environment_lock();
    let temp = TempDir::new().expect("create concurrent-writers tempfile");
    let state_home = temp.path().join("state");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root, _resolved, mut scope) = workspace(temp.path(), "concurrent-writers");
    scope.provider_workspace_identity_digest = format!("{:064x}", 17);
    let registry = Arc::new(WorkspaceDbRegistry::default());
    let session = registry
        .acquire(&project_root, &scope)
        .await
        .expect("acquire writer test workspace");
    let mut writes = tokio::task::JoinSet::new();
    for index in 0..100 {
        let session = session.clone();
        let scope = scope.clone();
        writes.spawn(async move {
            session
                .write_provider_incremental_owner(&ProviderIncrementalOwnerWrite {
                    scope,
                    owner_path: format!("src/owner-{index:03}.rs"),
                    fingerprint: ProviderOwnerFingerprint {
                        metadata: ProviderOwnerMetadata {
                            file_identity: format!("writer-file-{index}"),
                            size_bytes: 1,
                            modified_unix_nanos: index,
                            change_time_unix_nanos: index,
                        },
                        content_digest: format!("{:064x}", index + 1),
                    },
                    projection_completeness: "complete-owner".to_owned(),
                    projections: Vec::new(),
                })
                .await
        });
    }
    let mut completed = 0;
    while let Some(write) = writes.join_next().await {
        write
            .expect("concurrent writer task must join")
            .expect("concurrent writer transaction must commit");
        completed += 1;
    }

    assert_eq!(completed, 100);
    let counters = registry.counters();
    assert_eq!(counters.database_open_count, 1);
    assert_eq!(counters.connection_create_count, 2);
    assert_eq!(counters.schema_bootstrap_count, 1);
    assert_eq!(counters.writer_transaction_count, 100);
    assert_eq!(counters.max_active_writer_count, 1);
}
