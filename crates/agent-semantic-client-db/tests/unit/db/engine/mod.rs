use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Barrier},
    thread,
};

use agent_semantic_client_core::state_core::{
    ASP_STATE_HOME_ENV, ResolvedState, STATE_LAYOUT_VERSION, TURSO_BACKEND,
};
use agent_semantic_client_core::{CacheExportMethod, ClientCacheManifest, LanguageId, ProviderId};
use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, SemanticSchemaId, SemanticSchemaVersion,
};
use agent_semantic_client_db::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, ClientDbSourceIndexImportFile,
    ClientDbSourceIndexImportRequest, ClientDbSourceIndexLookupState, ClientDbSourceIndexSource,
    ClientDbStructuralDependencyUsage, ClientDbStructuralIndexImport, ClientDbStructuralKind,
    ClientDbStructuralLocator, ClientDbStructuralName, ClientDbStructuralOwner,
    ClientDbStructuralPath, ClientDbStructuralQueryKey, ClientDbStructuralSource,
    ClientDbStructuralSymbol, build_source_index_import,
};
use agent_semantic_client_db::{ClientDbArtifactEvent, ClientDbBackend, ClientDbEngine};
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
include!("turso_agent_storage.rs");

mod turso_mvcc_keyset_tests {
    include!("turso_mvcc_keyset.rs");
}
include!("write_session.rs");

fn temp_root(label: &str) -> PathBuf {
    let mut root = std::env::temp_dir();
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

struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
    _env_lock: std::sync::MutexGuard<'static, ()>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let env_lock = crate::env::ENV_LOCK.lock().expect("lock env");
        let previous = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self {
            key,
            previous,
            _env_lock: env_lock,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => unsafe {
                std::env::set_var(self.key, value);
            },
            None => unsafe {
                std::env::remove_var(self.key);
            },
        }
    }
}
