use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use agent_semantic_client_core::state_core::ResolvedState;
use agent_semantic_client_db::{
    ProviderIncrementalOwnerWriteV1, ProviderIncrementalScopeV1, ProviderOwnerFingerprintV1,
    ProviderOwnerMetadataV1, WorkspaceDbRegistry, WorkspaceDbRegistryCountersV1,
};

pub(super) struct TestDir(PathBuf);

impl TestDir {
    pub(super) fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "asp-workspace-db-registry-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            label
        ));
        std::fs::create_dir_all(&path).expect("create workspace registry test directory");
        Self(path)
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn environment_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("lock workspace registry test environment")
}

pub(super) struct StateHomeGuard {
    previous: Option<OsString>,
}

impl StateHomeGuard {
    pub(super) fn install(state_home: &Path) -> Self {
        let previous = std::env::var_os("AST_STATE_HOME");
        unsafe {
            std::env::set_var("AST_STATE_HOME", state_home);
        }
        Self { previous }
    }
}

impl Drop for StateHomeGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(previous) = &self.previous {
                std::env::set_var("AST_STATE_HOME", previous);
            } else {
                std::env::remove_var("AST_STATE_HOME");
            }
        }
    }
}

pub(super) fn workspace(
    parent: &Path,
    name: &str,
) -> (PathBuf, ResolvedState, ProviderIncrementalScopeV1) {
    let project_root = parent.join(name);
    std::fs::create_dir_all(&project_root).expect("create workspace registry project");
    let resolved = ResolvedState::resolve(&project_root).expect("resolve workspace registry state");
    let scope = ProviderIncrementalScopeV1 {
        project_root: project_root.to_string_lossy().into_owned(),
        workspace_identity: resolved.workspace.workspace_id.as_str().to_owned(),
        provider_workspace_identity_digest: "provider-workspace-v1".to_owned(),
        language_id: "rust".to_owned(),
        provider_id: "rust-test-provider".to_owned(),
        provider_workspace_root: project_root.to_string_lossy().into_owned(),
    };
    (project_root, resolved, scope)
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_workspace_identity_fails_before_database_open() {
    let _environment = environment_lock();
    let temp = TestDir::new("wrong-identity");
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
    assert_eq!(registry.counters(), WorkspaceDbRegistryCountersV1::default());
    assert!(!client_db_path.exists());
}

#[tokio::test(flavor = "current_thread")]
async fn one_hundred_concurrent_leases_open_and_bootstrap_once() {
    let _environment = environment_lock();
    let temp = TestDir::new("concurrent");
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
    let temp = TestDir::new("independent");
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
    let temp = TestDir::new("concurrent-writers");
    let state_home = temp.path().join("state");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root, _resolved, mut scope) =
        workspace(temp.path(), "concurrent-writers");
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
                .write_provider_incremental_owner(&ProviderIncrementalOwnerWriteV1 {
                    scope,
                    owner_path: format!("src/owner-{index:03}.rs"),
                    fingerprint: ProviderOwnerFingerprintV1 {
                        metadata: ProviderOwnerMetadataV1 {
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
