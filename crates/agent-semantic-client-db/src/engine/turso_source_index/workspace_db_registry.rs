//! Process-resident Turso resources partitioned by canonical workspace identity.

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use agent_semantic_client_core::state_core::ResolvedState;
use parking_lot::Mutex;
use tokio::sync::{Mutex as AsyncMutex, OnceCell};

use super::ProviderIncrementalScopeV1;
use super::{
    ProviderOwnerBatchProbeReceiptV1, ProviderOwnerBatchProbeRequestV1,
    ProviderIncrementalOwnerWriteV1, ProviderIncrementalWriteReceiptV1,
    ProviderOwnerInventoryWriteReceiptV1, ProviderOwnerInventoryWriteV1,
    ProviderTreeSitterOwnerResultV1, ProviderTreeSitterOwnerWriteReceiptV1,
    ProviderTreeSitterQueryIdentityV1,
};

/// Process-lifetime counters for workspace database resource reuse.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceDbRegistryCountersV1 {
    pub database_open_count: u64,
    pub connection_create_count: u64,
    pub schema_bootstrap_count: u64,
    pub registry_hit_count: u64,
    pub workspace_lock_retry_count: u64,
    pub writer_transaction_count: u64,
    pub max_active_writer_count: u64,
}

/// A process-resident registry with one independent initialization slot per workspace.
#[derive(Debug, Default)]
pub struct WorkspaceDbRegistry {
    slots: Mutex<HashMap<String, Arc<WorkspaceDbSlot>>>,
    database_open_count: AtomicU64,
    connection_create_count: AtomicU64,
    schema_bootstrap_count: AtomicU64,
    registry_hit_count: AtomicU64,
}

#[derive(Debug)]
struct WorkspaceDbSlot {
    workspace_identity: String,
    client_db_path: PathBuf,
    entry: OnceCell<Arc<WorkspaceDbEntry>>,
}

#[derive(Debug)]
struct WorkspaceDbEntry {
    workspace_identity: String,
    client_db_path: PathBuf,
    _database: turso::Database,
    read_connection: turso::connection::Connection,
    writer_connection: AsyncMutex<turso::connection::Connection>,
    writer_transaction_count: AtomicU64,
    active_writer_count: AtomicU64,
    max_active_writer_count: AtomicU64,
}

/// A command-scoped lease over process-resident resources for one workspace.
#[derive(Clone, Debug)]
pub struct ProviderSearchWorkspaceSessionV1 {
    entry: Arc<WorkspaceDbEntry>,
}

pub(super) struct ProviderSearchWriterGuard<'a> {
    guard: tokio::sync::MutexGuard<'a, turso::connection::Connection>,
    active_writer_count: &'a AtomicU64,
}

impl Deref for ProviderSearchWriterGuard<'_> {
    type Target = turso::connection::Connection;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl DerefMut for ProviderSearchWriterGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

impl Drop for ProviderSearchWriterGuard<'_> {
    fn drop(&mut self) {
        self.active_writer_count.fetch_sub(1, Ordering::Relaxed);
    }
}

impl WorkspaceDbRegistry {
    /// Return the process registry. Entries remain partitioned by workspace identity.
    pub fn process() -> &'static Self {
        static REGISTRY: OnceLock<WorkspaceDbRegistry> = OnceLock::new();
        REGISTRY.get_or_init(Self::default)
    }

    /// Resolve and validate canonical workspace ownership before any database open.
    pub async fn acquire(
        &self,
        project_root: impl AsRef<Path>,
        scope: &ProviderIncrementalScopeV1,
    ) -> Result<ProviderSearchWorkspaceSessionV1, String> {
        let project_root = project_root.as_ref();
        let resolved = ResolvedState::resolve(project_root)?;
        self.acquire_resolved(project_root, scope, resolved).await
    }

    async fn acquire_resolved(
        &self,
        project_root: &Path,
        scope: &ProviderIncrementalScopeV1,
        resolved: ResolvedState,
    ) -> Result<ProviderSearchWorkspaceSessionV1, String> {
        let resolved_workspace_identity = resolved.workspace.workspace_id.as_str();
        let canonical_project_root = project_root.canonicalize().map_err(|error| {
            format!(
                "failed to canonicalize provider search project root {}: {error}",
                project_root.display()
            )
        })?;
        let canonical_scope_root =
            Path::new(&scope.project_root)
                .canonicalize()
                .map_err(|error| {
                    format!(
                        "failed to canonicalize provider incremental scope root {}: {error}",
                        scope.project_root
                    )
                })?;
        if canonical_project_root != resolved.workspace.root
            || canonical_scope_root != resolved.workspace.root
        {
            return Err(format!(
                "provider incremental scope project root does not resolve to workspace {}",
                resolved_workspace_identity
            ));
        }
        if scope.workspace_identity != resolved_workspace_identity {
            return Err(format!(
                "provider incremental scope workspace identity mismatch: resolved={} requested={}",
                resolved_workspace_identity, scope.workspace_identity
            ));
        }

        let client_db_path = resolved.paths.client_db_path;
        let slot = {
            let mut slots = self.slots.lock();
            if let Some(slot) = slots.get(resolved_workspace_identity) {
                if slot.workspace_identity != resolved_workspace_identity
                    || slot.client_db_path != client_db_path
                {
                    return Err(format!(
                        "workspace registry identity/path mismatch for {}",
                        resolved_workspace_identity
                    ));
                }
                self.registry_hit_count.fetch_add(1, Ordering::Relaxed);
                Arc::clone(slot)
            } else {
                let slot = Arc::new(WorkspaceDbSlot {
                    workspace_identity: resolved_workspace_identity.to_owned(),
                    client_db_path: client_db_path.clone(),
                    entry: OnceCell::new(),
                });
                slots.insert(resolved_workspace_identity.to_owned(), Arc::clone(&slot));
                slot
            }
        };

        let entry = slot
            .entry
            .get_or_try_init(|| async {
                if let Some(parent) = client_db_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|error| {
                        format!(
                            "failed to create canonical workspace client DB directory {}: {error}",
                            parent.display()
                        )
                    })?;
                }
                let path = client_db_path.to_str().ok_or_else(|| {
                    format!(
                        "canonical workspace client DB path is not UTF-8: {}",
                        client_db_path.display()
                    )
                })?;
                let database = turso::Builder::new_local(path)
                    .build()
                    .await
                    .map_err(|error| {
                        format!(
                            "failed to open canonical workspace Turso database {}: {error}",
                            client_db_path.display()
                        )
                    })?;
                self.database_open_count.fetch_add(1, Ordering::Relaxed);
                let read_connection = database.connect().map_err(|error| {
                    format!(
                        "failed to create canonical workspace Turso read connection {}: {error}",
                        client_db_path.display()
                    )
                })?;
                let writer_connection = database.connect().map_err(|error| {
                    format!(
                        "failed to create canonical workspace Turso writer connection {}: {error}",
                        client_db_path.display()
                    )
                })?;
                self.connection_create_count
                    .fetch_add(2, Ordering::Relaxed);
                super::bootstrap_turso_source_index_schema(&writer_connection).await?;
                self.schema_bootstrap_count
                    .fetch_add(1, Ordering::Relaxed);
                Ok::<_, String>(Arc::new(WorkspaceDbEntry {
                    workspace_identity: resolved_workspace_identity.to_owned(),
                    client_db_path: client_db_path.clone(),
                    _database: database,
                    read_connection,
                    writer_connection: AsyncMutex::new(writer_connection),
                    writer_transaction_count: AtomicU64::new(0),
                    active_writer_count: AtomicU64::new(0),
                    max_active_writer_count: AtomicU64::new(0),
                }))
            })
            .await?;

        Ok(ProviderSearchWorkspaceSessionV1 {
            entry: Arc::clone(entry),
        })
    }

    pub fn counters(&self) -> WorkspaceDbRegistryCountersV1 {
        let (writer_transaction_count, max_active_writer_count) = {
            let slots = self.slots.lock();
            slots
                .values()
                .filter_map(|slot| slot.entry.get())
                .fold((0_u64, 0_u64), |(transactions, max_active), entry| {
                    (
                        transactions
                            + entry.writer_transaction_count.load(Ordering::Relaxed),
                        max_active.max(entry.max_active_writer_count.load(Ordering::Relaxed)),
                    )
                })
        };
        WorkspaceDbRegistryCountersV1 {
            database_open_count: self.database_open_count.load(Ordering::Relaxed),
            connection_create_count: self.connection_create_count.load(Ordering::Relaxed),
            schema_bootstrap_count: self.schema_bootstrap_count.load(Ordering::Relaxed),
            registry_hit_count: self.registry_hit_count.load(Ordering::Relaxed),
            workspace_lock_retry_count: 0,
            writer_transaction_count,
            max_active_writer_count,
        }
    }
}

impl ProviderSearchWorkspaceSessionV1 {
    pub async fn write_provider_incremental_owner(
        &self,
        request: &ProviderIncrementalOwnerWriteV1,
    ) -> Result<ProviderIncrementalWriteReceiptV1, String> {
        super::provider_incremental::write_provider_incremental_owner_in_session_v1(self, request)
            .await
    }

    pub async fn read_provider_treesitter_query(
        &self,
        query: &super::ProviderTreeSitterQueryIdentityV1,
        incremental_budget: u32,
        continuation: Option<&super::ProviderTreeSitterContinuationV1>,
    ) -> Result<super::ProviderTreeSitterQueryReadV1, String> {
        super::provider_treesitter_read::read_provider_treesitter_query_in_session_v1(
            self,
            query,
            incremental_budget,
            continuation,
        )
        .await
    }

    pub fn workspace_identity(&self) -> &str {
        &self.entry.workspace_identity
    }

    pub fn client_db_path(&self) -> &Path {
        &self.entry.client_db_path
    }

    pub async fn probe_provider_owners(
        &self,
        scope: &ProviderIncrementalScopeV1,
        owners: &[ProviderOwnerBatchProbeRequestV1],
    ) -> Result<ProviderOwnerBatchProbeReceiptV1, String> {
        super::provider_incremental_probe_batch::probe_provider_owners_in_session_v1(
            self, scope, owners,
        )
        .await
    }

    pub async fn read_provider_owner_projections(
        &self,
        scope: &ProviderIncrementalScopeV1,
        owner_path: &str,
    ) -> Result<Vec<super::ProviderSelectorProjectionV1>, String> {
        super::provider_incremental::read_provider_owner_projections(
            self.read_connection(),
            scope,
            owner_path,
        )
        .await
    }

    pub async fn upsert_provider_owner_inventory(
        &self,
        request: &ProviderOwnerInventoryWriteV1,
    ) -> Result<ProviderOwnerInventoryWriteReceiptV1, String> {
        super::provider_treesitter_write::upsert_provider_owner_inventory_in_session_v1(
            self, request,
        )
        .await
    }

    pub async fn write_provider_treesitter_owner_result(
        &self,
        query: &ProviderTreeSitterQueryIdentityV1,
        result: &ProviderTreeSitterOwnerResultV1,
    ) -> Result<ProviderTreeSitterOwnerWriteReceiptV1, String> {
        super::provider_treesitter_write::write_provider_treesitter_owner_result_in_session_v1(
            self, query, result,
        )
        .await
    }

    pub(super) fn read_connection(&self) -> &turso::connection::Connection {
        &self.entry.read_connection
    }

    pub(super) async fn writer_connection(
        &self,
    ) -> ProviderSearchWriterGuard<'_> {
        let guard = self.entry.writer_connection.lock().await;
        self.entry
            .writer_transaction_count
            .fetch_add(1, Ordering::Relaxed);
        let active = self
            .entry
            .active_writer_count
            .fetch_add(1, Ordering::Relaxed)
            + 1;
        self.entry
            .max_active_writer_count
            .fetch_max(active, Ordering::Relaxed);
        ProviderSearchWriterGuard {
            guard,
            active_writer_count: &self.entry.active_writer_count,
        }
    }
}
