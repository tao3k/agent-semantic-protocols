//! Process-resident Turso resources partitioned by canonical workspace identity.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use agent_semantic_client_core::state_core::ResolvedState;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

use super::ProviderIncrementalScoped;
use super::workspace_db_owner::{
    WorkspaceDbWriteOperation, WorkspaceDbWriteRequest, WorkspaceDbWriteResult,
    WorkspaceDbWriterClient, run_workspace_db_writer_actor, workspace_db_writer_channel,
};
use super::{
    ProviderIncrementalOwnerWrite, ProviderIncrementalWriteReceipt, ProviderOwnerBatchProbeReceipt,
    ProviderOwnerBatchProbeRequest, ProviderOwnerInventoryWrite,
    ProviderOwnerInventoryWriteReceipt, ProviderTreeSitterOwnerResult,
    ProviderTreeSitterOwnerWriteReceipt, ProviderTreeSitterQueryIdentity,
};

/// Process-lifetime counters for workspace database resource reuse.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceDbRegistryCounters {
    pub database_open_count: u64,
    pub connection_create_count: u64,
    pub schema_bootstrap_count: u64,
    pub registry_hit_count: u64,
    pub workspace_resolution_count: u64,
    pub project_root_canonicalization_count: u64,
    pub workspace_lock_retry_count: u64,
    pub writer_transaction_count: u64,
    pub max_active_writer_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceDbWriteFinishMode {
    ResidentBatch,
    OwnerDurabilityBoundary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDbWriteFinishReceipt {
    pub cache_flush_count: u32,
    pub checkpoint_mode: WorkspaceDbWriteFinishMode,
    pub checkpoint_busy: Option<i64>,
    pub log_frames: Option<i64>,
    pub checkpointed_frames: Option<i64>,
}

/// A process-resident registry with one independent initialization slot per workspace.
#[derive(Debug, Default)]
pub struct WorkspaceDbRegistry {
    slots: Mutex<HashMap<String, Arc<WorkspaceDbSlot>>>,
    admission_sessions: Mutex<HashMap<String, Arc<OnceCell<ProviderSearchWorkspaceSession>>>>,
    database_open_count: AtomicU64,
    connection_create_count: Arc<AtomicU64>,
    schema_bootstrap_count: AtomicU64,
    registry_hit_count: AtomicU64,
    workspace_resolution_count: AtomicU64,
    project_root_canonicalization_count: AtomicU64,
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
    read_connections: Vec<Arc<turso::connection::Connection>>,
    active_reader_count: AtomicU64,
    next_read_connection: AtomicU64,
    source_index_read_cache: Vec<
        tokio::sync::Mutex<
            Option<(
                WorkspaceDbSourceIndexReadKey,
                crate::ClientDbSourceIndexLookupResult,
            )>,
        >,
    >,
    writer_client: WorkspaceDbWriterClient,
    writer_task: tokio::task::JoinHandle<()>,
    next_request_id: AtomicU64,
    writer_transaction_count: AtomicU64,
    max_active_writer_count: AtomicU64,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct WorkspaceDbSourceIndexReadKey {
    indexed_project_root: String,
    snapshot_root: String,
    provider_digest: String,
    query: String,
    language_id: Option<String>,
    limit: u32,
}

pub(super) struct WorkspaceDbReadLease<'a> {
    entry: &'a WorkspaceDbEntry,
    connection: Arc<turso::connection::Connection>,
}

impl std::ops::Deref for WorkspaceDbReadLease<'_> {
    type Target = turso::connection::Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl WorkspaceDbReadLease<'_> {
    fn shared_connection(&self) -> Arc<turso::Connection> {
        Arc::clone(&self.connection)
    }
}

impl Drop for WorkspaceDbReadLease<'_> {
    fn drop(&mut self) {
        self.entry
            .active_reader_count
            .fetch_sub(1, Ordering::Relaxed);
    }
}

impl Drop for WorkspaceDbEntry {
    fn drop(&mut self) {
        self.writer_task.abort();
    }
}

/// A command-scoped lease over process-resident resources for one workspace.
#[derive(Clone, Debug)]
pub struct ProviderSearchWorkspaceSession {
    entry: Arc<WorkspaceDbEntry>,
}

impl WorkspaceDbRegistry {
    pub fn workspace_entry_counts(&self) -> (usize, usize) {
        let (slot_count, loaded) = {
            let slots = self.slots.lock();
            (
                slots.len(),
                slots
                    .values()
                    .filter(|slot| slot.entry.get().is_some())
                    .count(),
            )
        };
        let admission_count = self.admission_sessions.lock().len();
        (slot_count.max(admission_count), loaded)
    }

    /// Resolve and validate canonical workspace ownership before any database open.
    pub async fn acquire(
        &self,
        project_root: impl AsRef<Path>,
        scope: &ProviderIncrementalScoped,
    ) -> Result<ProviderSearchWorkspaceSession, String> {
        let workspace_identity = scope.workspace_identity.trim();
        if workspace_identity.is_empty() {
            return Err("workspace admission requires a non-empty workspace identity".to_owned());
        }
        let project_root = project_root.as_ref();
        let (admission, reused_admission) = {
            let mut admissions = self.admission_sessions.lock();
            if let Some(admission) = admissions.get(workspace_identity) {
                (Arc::clone(admission), true)
            } else {
                let admission = Arc::new(OnceCell::new());
                admissions.insert(workspace_identity.to_owned(), Arc::clone(&admission));
                (admission, false)
            }
        };
        if reused_admission {
            self.registry_hit_count.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(session) = admission.get() {
            return Ok(session.clone());
        }
        let session = admission
            .get_or_try_init(|| async {
                self.workspace_resolution_count
                    .fetch_add(1, Ordering::Relaxed);
                let resolved = ResolvedState::resolve(project_root)?;
                self.acquire_resolved(project_root, scope, resolved).await
            })
            .await?;
        Ok(session.clone())
    }

    pub async fn bootstrap_workspace(
        &self,
        project_root: impl AsRef<Path>,
    ) -> Result<ProviderSearchWorkspaceSession, String> {
        self.workspace_resolution_count
            .fetch_add(1, Ordering::Relaxed);
        let resolved = ResolvedState::resolve(project_root)?;
        let entry = self
            .entry_for_resolved(
                resolved.workspace.workspace_id.as_str(),
                resolved.paths.client_db_path,
            )
            .await?;
        Ok(ProviderSearchWorkspaceSession { entry })
    }

    pub fn loaded_session(
        &self,
        workspace_identity: &str,
    ) -> Option<ProviderSearchWorkspaceSession> {
        let admitted = {
            let admissions = self.admission_sessions.lock();
            admissions
                .get(workspace_identity)
                .and_then(|admission| admission.get())
                .cloned()
        };
        if let Some(session) = admitted {
            self.registry_hit_count.fetch_add(1, Ordering::Relaxed);
            return Some(session);
        }
        let entry = {
            let slots = self.slots.lock();
            slots
                .get(workspace_identity)
                .and_then(|slot| slot.entry.get())
                .cloned()
        }?;
        self.registry_hit_count.fetch_add(1, Ordering::Relaxed);
        Some(ProviderSearchWorkspaceSession { entry })
    }

    async fn acquire_resolved(
        &self,
        project_root: &Path,
        scope: &ProviderIncrementalScoped,
        resolved: ResolvedState,
    ) -> Result<ProviderSearchWorkspaceSession, String> {
        let resolved_workspace_identity = resolved.workspace.workspace_id.as_str();
        if scope.workspace_identity != resolved_workspace_identity {
            return Err(format!(
                "provider incremental scope workspace identity mismatch: resolved={} requested={}",
                resolved_workspace_identity, scope.workspace_identity
            ));
        }
        self.project_root_canonicalization_count
            .fetch_add(1, Ordering::Relaxed);
        let canonical_project_root =
            tokio::fs::canonicalize(project_root)
                .await
                .map_err(|error| {
                    format!(
                        "failed to canonicalize provider search project root {}: {error}",
                        project_root.display()
                    )
                })?;
        let scope_root = Path::new(&scope.project_root);
        let canonical_scope_root = if scope_root == project_root {
            canonical_project_root.clone()
        } else {
            self.project_root_canonicalization_count
                .fetch_add(1, Ordering::Relaxed);
            tokio::fs::canonicalize(scope_root).await.map_err(|error| {
                format!(
                    "failed to canonicalize provider incremental scope root {}: {error}",
                    scope.project_root
                )
            })?
        };
        if !canonical_project_root.starts_with(&resolved.workspace.root)
            || !canonical_scope_root.starts_with(&resolved.workspace.root)
        {
            return Err(format!(
                "provider incremental scope project root is outside workspace {}: workspaceRoot={} projectRoot={} scopeRoot={}",
                resolved_workspace_identity,
                resolved.workspace.root.display(),
                canonical_project_root.display(),
                canonical_scope_root.display(),
            ));
        }
        let entry = self
            .entry_for_resolved(resolved_workspace_identity, resolved.paths.client_db_path)
            .await?;

        Ok(ProviderSearchWorkspaceSession { entry })
    }

    async fn entry_for_resolved(
        &self,
        resolved_workspace_identity: &str,
        client_db_path: PathBuf,
    ) -> Result<Arc<WorkspaceDbEntry>, String> {
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
                    tokio::fs::create_dir_all(parent).await.map_err(|error| {
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
                let read_parallelism = workspace_db_reader_connection_limit();
                let mut read_connections = Vec::with_capacity(read_parallelism);
                for _ in 0..read_parallelism {
                    read_connections.push(Arc::new(database.connect().map_err(|error| {
                        format!(
                            "failed to create canonical workspace Turso read connection {}: {error}",
                            client_db_path.display()
                        )
                    })?));
                }
                let writer_connection = database.connect().map_err(|error| {
                    format!(
                        "failed to create canonical workspace Turso writer connection {}: {error}",
                        client_db_path.display()
                    )
                })?;
                self.connection_create_count.fetch_add(
                    u64::try_from(read_parallelism.saturating_add(1)).unwrap_or(u64::MAX),
                    Ordering::Relaxed,
                );
                super::bootstrap_turso_source_index_schema(&writer_connection).await?;
                self.schema_bootstrap_count.fetch_add(1, Ordering::Relaxed);
                let (writer_queue_capacity, writer_batch_limit) =
                    workspace_db_writer_concurrency_plan();
                let (writer_client, writer_actor) =
                    workspace_db_writer_channel(writer_queue_capacity);
                let writer_task = tokio::spawn(run_workspace_db_writer_actor(
                    writer_actor,
                    writer_connection,
                    writer_batch_limit,
                ));
                Ok::<_, String>(Arc::new(WorkspaceDbEntry {
                    workspace_identity: resolved_workspace_identity.to_owned(),
                    client_db_path: client_db_path.clone(),
                    _database: database,
                    read_connections,
                    active_reader_count: AtomicU64::new(0),
                    next_read_connection: AtomicU64::new(0),
                    source_index_read_cache: (0..read_parallelism)
                        .map(|_| tokio::sync::Mutex::new(None))
                        .collect(),
                    writer_client,
                    writer_task,
                    next_request_id: AtomicU64::new(0),
                    writer_transaction_count: AtomicU64::new(0),
                    max_active_writer_count: AtomicU64::new(0),
                }))
            })
            .await?;

        Ok(Arc::clone(entry))
    }

    pub async fn finish_loaded_writes(
        &self,
        mode: crate::WorkspaceDbWriteFinishMode,
    ) -> Result<(), String> {
        let loaded_entries = {
            let slots = self.slots.lock();
            slots
                .values()
                .filter_map(|slot| slot.entry.get().cloned())
                .collect::<Vec<_>>()
        };
        for entry in loaded_entries {
            ProviderSearchWorkspaceSession { entry }
                .finish_writes(mode)
                .await?;
        }
        Ok(())
    }

    pub fn counters(&self) -> WorkspaceDbRegistryCounters {
        let (writer_transaction_count, max_active_writer_count) = {
            let slots = self.slots.lock();
            slots.values().filter_map(|slot| slot.entry.get()).fold(
                (0_u64, 0_u64),
                |(transactions, max_active), entry| {
                    (
                        transactions + entry.writer_transaction_count.load(Ordering::Relaxed),
                        max_active.max(entry.max_active_writer_count.load(Ordering::Relaxed)),
                    )
                },
            )
        };
        WorkspaceDbRegistryCounters {
            database_open_count: self.database_open_count.load(Ordering::Relaxed),
            connection_create_count: self.connection_create_count.load(Ordering::Relaxed),
            schema_bootstrap_count: self.schema_bootstrap_count.load(Ordering::Relaxed),
            registry_hit_count: self.registry_hit_count.load(Ordering::Relaxed),
            workspace_resolution_count: self.workspace_resolution_count.load(Ordering::Relaxed),
            project_root_canonicalization_count: self
                .project_root_canonicalization_count
                .load(Ordering::Relaxed),
            workspace_lock_retry_count: 0,
            writer_transaction_count,
            max_active_writer_count,
        }
    }
}

impl ProviderSearchWorkspaceSession {
    pub async fn read_source_index(
        &self,
        indexed_project_root: &Path,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        query: &str,
        language_id: Option<&agent_semantic_client_core::LanguageId>,
        limit: u32,
    ) -> Result<crate::ClientDbSourceIndexLookupResult, String> {
        let cache_key = WorkspaceDbSourceIndexReadKey {
            indexed_project_root: indexed_project_root.display().to_string(),
            snapshot_root: source_snapshot.root_digest.clone(),
            provider_digest: source_snapshot.provider_digest.clone(),
            query: query.to_owned(),
            language_id: language_id.map(|language| language.as_str().to_owned()),
            limit,
        };
        let mut cache_hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&cache_key, &mut cache_hasher);
        let cache_index = std::hash::Hasher::finish(&cache_hasher) as usize
            % self.entry.source_index_read_cache.len();
        let mut cached = self.entry.source_index_read_cache[cache_index].lock().await;
        if let Some((cached_key, cached_result)) = cached.as_ref()
            && cached_key == &cache_key
        {
            return Ok(cached_result.clone());
        }
        let read_lease = self.read_connection();
        let result = super::super::source_index_facade::lookup::
            lookup_source_index_read_model_in_resident_connection(
                self.client_db_path().to_path_buf(),
                read_lease.shared_connection(),
                indexed_project_root,
                source_snapshot,
                query,
                language_id,
                limit,
            )
            .await?;
        *cached = Some((cache_key, result.clone()));
        Ok(result)
    }

    pub async fn commit_source_index_generation(
        &self,
        request: crate::ClientDbSourceIndexRefreshRequest,
        materialization: crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    ) -> Result<crate::ClientDbSourceIndexRefreshReport, String> {
        match self
            .submit_write(WorkspaceDbWriteOperation::CommitSourceIndexGeneration {
                request,
                materialization,
            })
            .await?
        {
            WorkspaceDbWriteResult::SourceIndexGeneration(receipt) => Ok(receipt),
            _ => Err(
                "workspace writer returned an unexpected source-index generation result".to_owned(),
            ),
        }
    }

    pub async fn load_active_workspace_generation_materialization(
        &self,
    ) -> Result<Option<crate::runtime_server_workspace::WorkspaceCanonicalMaterialization>, String>
    {
        let read_lease = self.read_connection();
        super::materialization::load_active_workspace_generation_materialization(
            read_lease.shared_connection().as_ref(),
            self.workspace_identity(),
        )
        .await
    }

    pub async fn write_provider_incremental_owner(
        &self,
        request: &ProviderIncrementalOwnerWrite,
    ) -> Result<ProviderIncrementalWriteReceipt, String> {
        match self
            .submit_write(WorkspaceDbWriteOperation::WriteProviderOwner(
                request.clone(),
            ))
            .await?
        {
            WorkspaceDbWriteResult::ProviderOwner(receipt) => Ok(receipt),
            _ => Err("workspace owner returned mismatched provider owner receipt".to_owned()),
        }
    }

    pub async fn read_provider_treesitter_query(
        &self,
        query: &super::ProviderTreeSitterQueryIdentity,
        incremental_budget: u32,
        continuation: Option<&super::ProviderTreeSitterContinuation>,
    ) -> Result<super::ProviderTreeSitterQueryRead, String> {
        super::provider_treesitter_read::read_provider_treesitter_query_in_session(
            self,
            query,
            incremental_budget,
            continuation,
        )
        .await
    }

    pub async fn read_resident_selector(
        &self,
        request: &super::TursoResidentSelectorQuery,
    ) -> Result<Option<super::TursoResidentSelectorRead>, String> {
        super::resident_selector::read_turso_resident_selector(self, request).await
    }

    pub fn workspace_identity(&self) -> &str {
        &self.entry.workspace_identity
    }

    pub fn client_db_path(&self) -> &Path {
        &self.entry.client_db_path
    }

    /// Flush committed writer pages to Turso's durable WAL before a short-lived
    /// CLI process returns control to its caller.
    pub async fn finish_writes(
        &self,
        mode: WorkspaceDbWriteFinishMode,
    ) -> Result<WorkspaceDbWriteFinishReceipt, String> {
        match self
            .submit_write(WorkspaceDbWriteOperation::FinishWrites(mode))
            .await?
        {
            WorkspaceDbWriteResult::WriteFinish(receipt) => Ok(receipt),
            _ => Err("workspace owner returned mismatched durability receipt".to_owned()),
        }
    }

    pub async fn probe_provider_owners(
        &self,
        scope: &ProviderIncrementalScoped,
        owners: &[ProviderOwnerBatchProbeRequest],
    ) -> Result<ProviderOwnerBatchProbeReceipt, String> {
        super::provider_incremental_probe_batch::probe_provider_owners_in_session(
            self, scope, owners,
        )
        .await
    }

    pub async fn read_provider_owner_projections(
        &self,
        scope: &ProviderIncrementalScoped,
        owner_path: &str,
    ) -> Result<Vec<super::ProviderSelectorProjection>, String> {
        let read_lease = self.read_connection();
        super::provider_incremental::read_provider_owner_projections(&read_lease, scope, owner_path)
            .await
    }

    pub async fn upsert_provider_owner_inventory(
        &self,
        request: &ProviderOwnerInventoryWrite,
    ) -> Result<ProviderOwnerInventoryWriteReceipt, String> {
        super::provider_treesitter_write::validate_inventory_write(request)?;
        match self
            .submit_write(WorkspaceDbWriteOperation::UpsertProviderInventory(
                request.clone(),
            ))
            .await?
        {
            WorkspaceDbWriteResult::ProviderInventory(receipt) => Ok(receipt),
            _ => Err("workspace owner returned mismatched inventory receipt".to_owned()),
        }
    }

    pub async fn write_provider_treesitter_owner_result(
        &self,
        query: &ProviderTreeSitterQueryIdentity,
        result: &ProviderTreeSitterOwnerResult,
    ) -> Result<ProviderTreeSitterOwnerWriteReceipt, String> {
        super::provider_treesitter_write::validate_query_owner_write(query, result)?;
        match self
            .submit_write(WorkspaceDbWriteOperation::WriteTreeSitterOwner {
                query: query.clone(),
                result: result.clone(),
            })
            .await?
        {
            WorkspaceDbWriteResult::TreeSitterOwner(receipt) => Ok(receipt),
            _ => Err("workspace owner returned mismatched Tree-sitter receipt".to_owned()),
        }
    }

    async fn submit_write(
        &self,
        operation: WorkspaceDbWriteOperation,
    ) -> Result<WorkspaceDbWriteResult, String> {
        let sequence = self.entry.next_request_id.fetch_add(1, Ordering::Relaxed);
        self.entry
            .writer_transaction_count
            .fetch_add(1, Ordering::Relaxed);
        self.entry
            .max_active_writer_count
            .fetch_max(1, Ordering::Relaxed);
        let response = self
            .entry
            .writer_client
            .submit(WorkspaceDbWriteRequest {
                workspace_identity: self.entry.workspace_identity.clone(),
                request_id: format!("workspace-write-{sequence}"),
                idempotency_key: format!("workspace-write-{sequence}"),
                operation,
            })
            .await?;
        if response.admission.sequence != sequence {
            return Err("workspace owner admission sequence drift".to_owned());
        }
        Ok(response.result)
    }

    pub(super) fn read_connection(&self) -> WorkspaceDbReadLease<'_> {
        self.entry
            .active_reader_count
            .fetch_add(1, Ordering::Relaxed);
        let index = self
            .entry
            .next_read_connection
            .fetch_add(1, Ordering::Relaxed) as usize
            % self.entry.read_connections.len();
        let connection = Arc::clone(&self.entry.read_connections[index]);
        WorkspaceDbReadLease {
            entry: &self.entry,
            connection,
        }
    }
}

fn workspace_db_reader_connection_limit() -> usize {
    crate::runtime_concurrency::RuntimeConcurrencyPlan::current().reader_limit()
}

fn workspace_db_writer_concurrency_plan() -> (usize, usize) {
    let plan = crate::runtime_concurrency::RuntimeConcurrencyPlan::current();
    (plan.writer_queue_capacity(), plan.writer_batch_limit())
}

#[cfg(test)]
#[path = "../../../tests/unit/workspace_db_registry_adaptive.rs"]
mod workspace_db_registry_tests;
