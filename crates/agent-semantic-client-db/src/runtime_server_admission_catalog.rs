//! Durable control-plane catalog of source scopes admitted by the Global daemon.

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc, OnceLock, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::{io::AsyncWriteExt, sync::Mutex};

const SCHEMA_ID: &str = "agent.semantic-protocols.runtime-server-workspace-admission-catalog.v1";

#[derive(Debug)]
pub enum RuntimeWorkspaceAdmissionCatalogResolveError {
    RootNotAbsolute(PathBuf),
    Unavailable {
        path: PathBuf,
        source: std::io::Error,
    },
    WorkspaceNotAdmitted(PathBuf),
    Invalid(String),
}

impl std::fmt::Display for RuntimeWorkspaceAdmissionCatalogResolveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootNotAbsolute(root) => write!(
                formatter,
                "workspace admission locator root must be absolute: {}",
                root.display()
            ),
            Self::Unavailable { path, source } => write!(
                formatter,
                "workspace admission locator is unavailable at {}: {source}",
                path.display()
            ),
            Self::WorkspaceNotAdmitted(root) => write!(
                formatter,
                "canonical workspace scope is not admitted: projectRoot={}",
                root.display()
            ),
            Self::Invalid(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for RuntimeWorkspaceAdmissionCatalogResolveError {}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CatalogFileIdentity {
    device: u64,
    inode: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
}

#[cfg(not(unix))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct CatalogFileIdentity {
    length: u64,
    modified: Option<std::time::SystemTime>,
}

#[derive(Clone, Debug)]
struct MappedCatalogIndex {
    identity: CatalogFileIdentity,
    entries_by_root: Arc<BTreeMap<PathBuf, RuntimeWorkspaceAdmissionCatalogEntry>>,
}

static MAPPED_CATALOGS: OnceLock<RwLock<BTreeMap<PathBuf, MappedCatalogIndex>>> = OnceLock::new();

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWorkspaceAdmissionCatalogEntry {
    pub workspace_identity: String,
    pub project_root: PathBuf,
}

impl RuntimeWorkspaceAdmissionCatalogEntry {
    pub fn validate(&self) -> Result<(), String> {
        if self.workspace_identity.trim().is_empty() {
            return Err("workspace admission catalog identity must be non-empty".to_owned());
        }
        if !self.project_root.is_absolute() {
            return Err(format!(
                "workspace admission catalog root must be canonical and absolute: {}",
                self.project_root.display()
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeWorkspaceAdmissionCatalogDocument {
    schema_id: String,
    schema_version: String,
    entries: Vec<RuntimeWorkspaceAdmissionCatalogEntry>,
}

#[derive(Clone, Debug, Default)]
struct CatalogDurabilityState {
    durable_revision: u64,
    failed_revision: Option<u64>,
    error: Option<Arc<str>>,
}

#[derive(Clone, Debug)]
pub struct RuntimeWorkspaceAdmissionCatalog {
    path: PathBuf,
    entries: tokio::sync::watch::Sender<Arc<BTreeSet<RuntimeWorkspaceAdmissionCatalogEntry>>>,
    resident_revision: Arc<AtomicU64>,
    durability: tokio::sync::watch::Sender<CatalogDurabilityState>,
    resident_writer: Arc<parking_lot::Mutex<()>>,
    writer: Arc<Mutex<()>>,
}

impl RuntimeWorkspaceAdmissionCatalog {
    pub async fn load(path: PathBuf) -> Result<Self, String> {
        let entries = match tokio::fs::read(&path).await {
            Ok(bytes) => decode_catalog(&bytes)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeSet::new(),
            Err(error) => {
                return Err(format!(
                    "failed to read Runtime Server workspace admission catalog {}: {error}",
                    path.display()
                ));
            }
        };
        let (entries, _) = tokio::sync::watch::channel(Arc::new(entries));
        let (durability, _) = tokio::sync::watch::channel(CatalogDurabilityState::default());
        Ok(Self {
            path,
            entries,
            resident_revision: Arc::new(AtomicU64::new(0)),
            durability,
            resident_writer: Arc::new(parking_lot::Mutex::new(())),
            writer: Arc::new(Mutex::new(())),
        })
    }

    pub fn snapshot(&self) -> Arc<BTreeSet<RuntimeWorkspaceAdmissionCatalogEntry>> {
        Arc::clone(&self.entries.borrow())
    }

    /// Subscribes to identity-map changes in the control-plane catalog.
    ///
    /// Re-admitting the same workspace is deliberately silent. Source,
    /// activation, and configuration generations have their own authorities;
    /// using an identity catalog as their invalidation bus turns every request
    /// into a full snapshot refresh.
    pub fn subscribe(
        &self,
    ) -> tokio::sync::watch::Receiver<Arc<BTreeSet<RuntimeWorkspaceAdmissionCatalogEntry>>> {
        self.entries.subscribe()
    }

    pub async fn record(
        &self,
        entry: RuntimeWorkspaceAdmissionCatalogEntry,
    ) -> Result<bool, String> {
        let inserted = self.record_resident(entry)?;
        if inserted {
            self.publish_resident_snapshot().await?;
        }
        Ok(inserted)
    }

    pub fn record_resident(
        &self,
        entry: RuntimeWorkspaceAdmissionCatalogEntry,
    ) -> Result<bool, String> {
        entry.validate()?;
        let _resident_writer = self.resident_writer.lock();
        if self.entries.borrow().contains(&entry) {
            return Ok(false);
        }
        let mut entries = self.entries.borrow().as_ref().clone();
        if let Some(existing) = entries.iter().find(|existing| {
            existing.project_root == entry.project_root
                && existing.workspace_identity != entry.workspace_identity
        }) {
            return Err(format!(
                "workspace admission catalog root identity conflict: projectRoot={} existingWorkspaceIdentity={} requestedWorkspaceIdentity={}",
                entry.project_root.display(),
                existing.workspace_identity,
                entry.workspace_identity
            ));
        }
        if let Some(existing) = entries.iter().find(|existing| {
            existing.workspace_identity == entry.workspace_identity
                && existing.project_root != entry.project_root
        }) {
            return Err(format!(
                "workspace admission catalog identity already owns a canonical root: workspaceIdentity={} existingProjectRoot={} requestedProjectRoot={}",
                entry.workspace_identity,
                existing.project_root.display(),
                entry.project_root.display()
            ));
        }
        if !entries.insert(entry) {
            return Ok(false);
        }
        self.entries.send_replace(Arc::new(entries));
        self.resident_revision.fetch_add(1, Ordering::Release);
        Ok(true)
    }

    pub async fn publish_resident_snapshot(&self) -> Result<(), String> {
        let _writer = self.writer.lock().await;
        let entries = Arc::clone(&self.entries.borrow());
        let revision = self.resident_revision.load(Ordering::Acquire);
        match publish_catalog(&self.path, entries.as_ref()).await {
            Ok(()) => {
                self.durability.send_modify(|state| {
                    state.durable_revision = state.durable_revision.max(revision);
                    if state
                        .failed_revision
                        .is_some_and(|failed| failed <= state.durable_revision)
                    {
                        state.failed_revision = None;
                        state.error = None;
                    }
                });
                Ok(())
            }
            Err(error) => {
                self.durability.send_modify(|state| {
                    state.failed_revision = Some(revision);
                    state.error = Some(Arc::from(error.as_str()));
                });
                Err(error)
            }
        }
    }

    /// Waits for the current resident identity revision to become durable.
    ///
    /// Generation admission remains an in-memory sub-millisecond operation;
    /// callers that require a terminal control-plane postcondition wait on this
    /// receipt rather than polling the filesystem or sleeping.
    pub async fn wait_durable(&self) -> Result<(), String> {
        let target_revision = self.resident_revision.load(Ordering::Acquire);
        let mut durability = self.durability.subscribe();
        loop {
            let state = durability.borrow().clone();
            if state.durable_revision >= target_revision {
                return Ok(());
            }
            if state
                .failed_revision
                .is_some_and(|failed| failed >= target_revision)
            {
                return Err(state
                    .error
                    .as_deref()
                    .unwrap_or("workspace admission catalog durability publication failed")
                    .to_owned());
            }
            durability.changed().await.map_err(|_| {
                "workspace admission catalog durability receipt channel closed".to_owned()
            })?;
        }
    }

    /// Rebuilds a missing derived locator from the resident identity catalog.
    ///
    /// This is intentionally separate from [`Self::record`]: ordinary
    /// admissions must never probe the filesystem merely to rediscover that an
    /// already-admitted identity is unchanged.
    pub async fn repair_locator(&self) -> Result<bool, String> {
        let _writer = self.writer.lock().await;
        match tokio::fs::metadata(&self.path).await {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => Ok(false),
            Ok(_) | Err(_) => {
                let entries = Arc::clone(&self.entries.borrow());
                publish_catalog(&self.path, entries.as_ref()).await?;
                Ok(true)
            }
        }
    }

    /// Resolves an already-admitted canonical project root without Git discovery,
    /// canonicalization, database access, or a Runtime Server roundtrip.
    pub fn resolve_mapped(
        path: &std::path::Path,
        project_root: &std::path::Path,
    ) -> Result<RuntimeWorkspaceAdmissionCatalogEntry, RuntimeWorkspaceAdmissionCatalogResolveError>
    {
        if !project_root.is_absolute() {
            return Err(
                RuntimeWorkspaceAdmissionCatalogResolveError::RootNotAbsolute(
                    project_root.to_path_buf(),
                ),
            );
        }
        let file = std::fs::File::open(path).map_err(|source| {
            RuntimeWorkspaceAdmissionCatalogResolveError::Unavailable {
                path: path.to_path_buf(),
                source,
            }
        })?;
        let invalid = RuntimeWorkspaceAdmissionCatalogResolveError::Invalid;
        let identity = catalog_file_identity(&file).map_err(&invalid)?;
        if let Some(entries_by_root) = cached_catalog_entries(path, &identity).map_err(&invalid)? {
            return resolve_catalog_entry(entries_by_root.as_ref(), project_root);
        }
        // SAFETY: catalog publication is immutable and atomic. Existing mappings retain
        // the previous inode while a writer publishes the next complete catalog.
        let mapping = unsafe { memmap2::MmapOptions::new().map(&file) }.map_err(|error| {
            RuntimeWorkspaceAdmissionCatalogResolveError::Invalid(format!(
                "map workspace admission locator: {error}"
            ))
        })?;
        let entries_by_root = Arc::new(
            decode_catalog(mapping.as_ref())
                .map_err(&invalid)?
                .into_iter()
                .map(|entry| (entry.project_root.clone(), entry))
                .collect(),
        );
        cache_catalog_entries(path, identity, Arc::clone(&entries_by_root)).map_err(&invalid)?;
        resolve_catalog_entry(entries_by_root.as_ref(), project_root)
    }
}

fn cached_catalog_entries(
    path: &std::path::Path,
    identity: &CatalogFileIdentity,
) -> Result<Option<Arc<BTreeMap<PathBuf, RuntimeWorkspaceAdmissionCatalogEntry>>>, String> {
    let catalogs = MAPPED_CATALOGS.get_or_init(Default::default);
    let catalogs = catalogs
        .read()
        .map_err(|_| "workspace admission locator cache read lock is poisoned".to_owned())?;
    Ok(catalogs
        .get(path)
        .filter(|catalog| catalog.identity == *identity)
        .map(|catalog| Arc::clone(&catalog.entries_by_root)))
}

fn cache_catalog_entries(
    path: &std::path::Path,
    identity: CatalogFileIdentity,
    entries_by_root: Arc<BTreeMap<PathBuf, RuntimeWorkspaceAdmissionCatalogEntry>>,
) -> Result<(), String> {
    let catalogs = MAPPED_CATALOGS.get_or_init(Default::default);
    catalogs
        .write()
        .map_err(|_| "workspace admission locator cache write lock is poisoned".to_owned())?
        .insert(
            path.to_path_buf(),
            MappedCatalogIndex {
                identity,
                entries_by_root,
            },
        );
    Ok(())
}

fn resolve_catalog_entry(
    entries_by_root: &BTreeMap<PathBuf, RuntimeWorkspaceAdmissionCatalogEntry>,
    project_root: &std::path::Path,
) -> Result<RuntimeWorkspaceAdmissionCatalogEntry, RuntimeWorkspaceAdmissionCatalogResolveError> {
    project_root
        .ancestors()
        .find_map(|candidate| entries_by_root.get(candidate).cloned())
        .ok_or_else(|| {
            RuntimeWorkspaceAdmissionCatalogResolveError::WorkspaceNotAdmitted(
                project_root.to_path_buf(),
            )
        })
}

#[cfg(unix)]
fn catalog_file_identity(file: &std::fs::File) -> Result<CatalogFileIdentity, String> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file
        .metadata()
        .map_err(|error| format!("read workspace admission locator metadata: {error}"))?;
    Ok(CatalogFileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        length: metadata.len(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
    })
}

#[cfg(not(unix))]
fn catalog_file_identity(file: &std::fs::File) -> Result<CatalogFileIdentity, String> {
    let metadata = file
        .metadata()
        .map_err(|error| format!("read workspace admission locator metadata: {error}"))?;
    Ok(CatalogFileIdentity {
        length: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn decode_catalog(bytes: &[u8]) -> Result<BTreeSet<RuntimeWorkspaceAdmissionCatalogEntry>, String> {
    let document = serde_json::from_slice::<RuntimeWorkspaceAdmissionCatalogDocument>(bytes)
        .map_err(|error| format!("decode Runtime Server workspace admission catalog: {error}"))?;
    if document.schema_id != SCHEMA_ID || document.schema_version != "1" {
        return Err("Runtime Server workspace admission catalog schema mismatch".to_owned());
    }
    let mut entries = BTreeSet::new();
    let mut root_owners = BTreeMap::<PathBuf, String>::new();
    let mut workspace_roots = BTreeMap::<String, PathBuf>::new();
    for entry in document.entries {
        entry.validate()?;
        if root_owners
            .get(&entry.project_root)
            .is_some_and(|identity| identity != &entry.workspace_identity)
        {
            return Err(format!(
                "Runtime Server workspace admission catalog contains a root identity conflict: projectRoot={}",
                entry.project_root.display()
            ));
        }
        if workspace_roots
            .get(&entry.workspace_identity)
            .is_some_and(|root| root != &entry.project_root)
        {
            return Err(format!(
                "Runtime Server workspace admission catalog maps one workspace identity to multiple roots: workspaceIdentity={}",
                entry.workspace_identity
            ));
        }
        let project_root = entry.project_root.clone();
        let workspace_identity = entry.workspace_identity.clone();
        if !entries.insert(entry) {
            return Err(
                "Runtime Server workspace admission catalog contains duplicates".to_owned(),
            );
        }
        root_owners.insert(project_root.clone(), workspace_identity.clone());
        workspace_roots.insert(workspace_identity, project_root);
    }
    Ok(entries)
}

async fn publish_catalog(
    path: &std::path::Path,
    entries: &BTreeSet<RuntimeWorkspaceAdmissionCatalogEntry>,
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "workspace admission catalog path has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent).await.map_err(|error| {
        format!(
            "create Runtime Server workspace admission catalog directory {}: {error}",
            parent.display()
        )
    })?;
    let document = RuntimeWorkspaceAdmissionCatalogDocument {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        entries: entries.iter().cloned().collect(),
    };
    let bytes = serde_json::to_vec(&document)
        .map_err(|error| format!("encode Runtime Server workspace admission catalog: {error}"))?;
    let pending = path.with_extension("pending");
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&pending)
        .await
        .map_err(|error| format!("open workspace admission catalog pending file: {error}"))?;
    file.write_all(&bytes)
        .await
        .map_err(|error| format!("write workspace admission catalog: {error}"))?;
    // This catalog is a derived locator, not the canonical generation authority.
    // Complete bytes plus atomic rename prevent torn readers; if the locator is
    // lost across a crash, the Runtime Server reconstructs it from the validated
    // immutable generation pointer through typed IPC.
    file.flush()
        .await
        .map_err(|error| format!("flush workspace admission catalog: {error}"))?;
    drop(file);
    tokio::fs::rename(&pending, path)
        .await
        .map_err(|error| format!("publish workspace admission catalog: {error}"))
}

#[cfg(test)]
#[path = "../tests/unit/runtime_server_admission_catalog.rs"]
mod tests;
