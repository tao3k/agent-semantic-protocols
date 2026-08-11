use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Barrier},
    thread,
};

use agent_semantic_client_core::state_core::{ResolvedState, STATE_LAYOUT_VERSION, TURSO_BACKEND};
use agent_semantic_client_core::{CacheExportMethod, ClientCacheManifest, LanguageId, ProviderId};
use agent_semantic_client_core::{CacheGenerationId, SemanticSchemaId, SemanticSchemaVersion};
use agent_semantic_client_db::{ClientDbArtifactEvent, ClientDbBackend, ClientDbEngine};
use agent_semantic_client_db::{
    ClientDbStructuralDependencyUsage, ClientDbStructuralIndexImport, ClientDbStructuralKind,
    ClientDbStructuralLocator, ClientDbStructuralName, ClientDbStructuralOwner,
    ClientDbStructuralPath, ClientDbStructuralQueryKey, ClientDbStructuralSource,
    ClientDbStructuralSymbol,
};
use serde_json::json;

include!("artifact_events.rs");
include!("bootstrap.rs");
include!("corrupt_cache.rs");
include!("contract.rs");
include!("turso_mvcc_store.rs");
include!("artifact_pointer.rs");
include!("artifact_pointer_domains.rs");
include!("artifact_pointer_crash.rs");
include!("turso_sync_storage.rs");
include!("turso_sync_server_e2e.rs");
include!("turso_cdc_storage.rs");
include!("turso_encrypted_storage.rs");
include!("storage_performance_receipt.rs");
include!("storage_contract.rs");
include!("turso_migration.rs");
include!("turso_agent_storage.rs");

mod turso_mvcc_keyset_tests {
    include!("turso_mvcc_keyset.rs");
}
include!("write_session.rs");

fn temp_root(label: &str) -> PathBuf {
    let repository = gix::discover(env!("CARGO_MANIFEST_DIR"))
        .expect("discover owner-backed database test repository with Gix");
    let mut root = repository
        .worktree()
        .expect("database tests require a non-bare owner checkout")
        .base()
        .join("target/asp-live-project-fixtures");
    let unique = format!(
        "asp-client-db-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    );
    root.push(unique);
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn init_git_repository(root: &Path) {
    let repository = gix::discover(root).expect("resolve owner repository with Gix");
    assert!(
        repository.worktree().is_some(),
        "database fixture must remain inside an owner-backed worktree"
    );
}
