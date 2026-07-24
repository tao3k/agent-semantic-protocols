use std::sync::Arc;

use agent_semantic_client_db::storage_contract::{
    AgentStorage, InMemoryAgentStorage, SessionEventPageRequest, StorageOptimizationProfile,
    StoragePartitionKey,
};

use crate::provider_runtime_storage::{
    ProviderExecutionStorageEvent, ProviderRuntimeStorageAdapter, ProviderRuntimeStorageBinding,
    ProviderRuntimeStorageContext,
};

struct TestProject(std::path::PathBuf);

impl TestProject {
    fn new(label: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "asp-provider-runtime-storage-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create temporary project");
        let status = std::process::Command::new("git")
            .arg("init")
            .arg("--quiet")
            .arg(&path)
            .status()
            .expect("run git init");
        assert!(status.success());
        Self(path)
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn context() -> ProviderRuntimeStorageContext {
    ProviderRuntimeStorageContext {
        repo_id: "repo".to_owned(),
        workspace_id: "workspace".to_owned(),
        scope_id: "scope".to_owned(),
        session_id: "session".to_owned(),
        root_session_id: "root-session".to_owned(),
        agent_id: "codex".to_owned(),
        invocation_id: "invocation-0001".to_owned(),
    }
}

#[test]
fn provider_runtime_storage_maps_runtime_identity_into_one_atomic_batch() {
    let storage = Arc::new(InMemoryAgentStorage::default());
    let adapter = ProviderRuntimeStorageAdapter::new(
        storage.clone(),
        StorageOptimizationProfile::CompatibilityImmediate,
    )
    .expect("construct provider runtime storage adapter");
    let event = ProviderExecutionStorageEvent::from_output(
        "cache-hit",
        "search",
        "rust",
        0,
        b"stdout",
        b"stderr",
        true,
        "root-session",
    );
    let receipt = adapter
        .append_provider_execution(&context(), &event, 42)
        .expect("append provider execution event");
    assert_eq!(receipt.committed_rows, 1);
    assert_eq!(
        receipt.optimization_profile,
        StorageOptimizationProfile::CompatibilityImmediate
    );

    let page = agent_semantic_runtime::runtime_block_on_current_thread(
        storage.list_session_events(&SessionEventPageRequest {
            partition: StoragePartitionKey {
                repo_id: "repo".to_owned(),
                workspace_id: "workspace".to_owned(),
                scope_id: "scope".to_owned(),
                session_id: "session".to_owned(),
                agent_id: "codex".to_owned(),
            },
            after: None,
            limit: 10,
        }),
    )
    .expect("runtime bridge")
    .expect("read recorded provider event");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].turn_id, "invocation-0001");
    assert_eq!(page.items[0].event_kind, "provider-execution:cache-hit");
    let decoded: ProviderExecutionStorageEvent =
        serde_json::from_slice(&page.items[0].payload).expect("decode provider event");
    assert_eq!(decoded, event);
    assert_eq!(decoded.root_session_id, "root-session");
}

#[test]
fn provider_runtime_storage_rejects_invalid_identity_without_partial_write() {
    let storage = Arc::new(InMemoryAgentStorage::default());
    let adapter = ProviderRuntimeStorageAdapter::new(
        storage.clone(),
        StorageOptimizationProfile::CompatibilityImmediate,
    )
    .expect("construct provider runtime storage adapter");
    let mut invalid = context();
    invalid.session_id.clear();
    let event = ProviderExecutionStorageEvent::from_output(
        "final",
        "query",
        "rust",
        1,
        &[],
        b"failed",
        false,
        "root-session",
    );
    let error = adapter
        .append_provider_execution(&invalid, &event, 43)
        .expect_err("empty session identity must fail closed");
    assert!(error.contains("InvalidRequest"));

    let page = agent_semantic_runtime::runtime_block_on_current_thread(
        storage.list_session_events(&SessionEventPageRequest {
            partition: StoragePartitionKey {
                repo_id: "repo".to_owned(),
                workspace_id: "workspace".to_owned(),
                scope_id: "scope".to_owned(),
                session_id: "session".to_owned(),
                agent_id: "codex".to_owned(),
            },
            after: None,
            limit: 10,
        }),
    )
    .expect("runtime bridge")
    .expect("read storage after failed append");
    assert!(page.items.is_empty());
}

#[test]
fn provider_runtime_storage_real_binding_persists_to_isolated_turso_profile() {
    let project = TestProject::new("real-binding");
    let binding = ProviderRuntimeStorageBinding::from_runtime_identity(
        &project.0,
        "codex",
        "session-real",
        "root-real",
    )
    .expect("open real production storage binding");
    let receipt = binding
        .record_invocation_start("search", "rust")
        .expect("record real provider invocation start");
    assert_eq!(receipt.backend, "turso");
    assert_eq!(receipt.backend_version, "0.7.0");
    assert_eq!(
        receipt.optimization_profile,
        StorageOptimizationProfile::MvccConcurrentPassiveCheckpoint
    );
    assert_eq!(receipt.committed_rows, 1);
    assert!(binding.event_db_path().exists());

    let event_db_path = binding.event_db_path().to_owned();
    let recorded_context = binding.context.clone();
    drop(binding);
    let mut config =
        agent_semantic_client_db::turso_mvcc_store::TursoMvccStoreConfig::new(event_db_path);
    config.connection_lanes = 4;
    config.passive_checkpoint = true;
    let reopened = agent_semantic_runtime::runtime_block_on_current_thread(
        agent_semantic_client_db::turso_agent_storage::TursoMvccAgentStorage::open(config),
    )
    .expect("runtime bridge")
    .expect("reopen production event database");
    let page = agent_semantic_runtime::runtime_block_on_current_thread(
        reopened.list_session_events(&SessionEventPageRequest {
            partition: StoragePartitionKey {
                repo_id: recorded_context.repo_id,
                workspace_id: recorded_context.workspace_id,
                scope_id: recorded_context.scope_id,
                session_id: recorded_context.session_id,
                agent_id: recorded_context.agent_id,
            },
            after: None,
            limit: 10,
        }),
    )
    .expect("runtime bridge")
    .expect("read persisted provider lifecycle event");
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.items[0].event_kind,
        "provider-execution:invocation-start"
    );
}
