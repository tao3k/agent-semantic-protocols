//! Durable control-plane catalog of source scopes admitted by the Global daemon.

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{Arc, OnceLock, RwLock},
};
use tokio::{io::AsyncWriteExt, sync::Mutex};

const SCHEMA_ID: &str = "agent.semantic-protocols.runtime-server-workspace-admission-catalog.v1";

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

#[derive(Clone, Debug)]
pub struct RuntimeWorkspaceAdmissionCatalog {
    path: PathBuf,
    entries: Arc<Mutex<BTreeSet<RuntimeWorkspaceAdmissionCatalogEntry>>>,
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
        Ok(Self {
            path,
            entries: Arc::new(Mutex::new(entries)),
        })
    }

    pub async fn entries(&self) -> Vec<RuntimeWorkspaceAdmissionCatalogEntry> {
        self.entries.lock().await.iter().cloned().collect()
    }

    pub async fn record(
        &self,
        entry: RuntimeWorkspaceAdmissionCatalogEntry,
    ) -> Result<bool, String> {
        entry.validate()?;
        let mut entries = self.entries.lock().await;
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
        let inserted_entry = entry.clone();
        if !entries.insert(entry) {
            return Ok(false);
        }
        if let Err(error) = publish_catalog(&self.path, &entries).await {
            entries.remove(&inserted_entry);
            return Err(error);
        }
        Ok(true)
    }

    /// Resolves an already-admitted canonical project root without Git discovery,
    /// canonicalization, database access, or a Runtime Server roundtrip.
    pub fn resolve_mapped(
        path: &std::path::Path,
        project_root: &std::path::Path,
    ) -> Result<RuntimeWorkspaceAdmissionCatalogEntry, String> {
        if !project_root.is_absolute() {
            return Err("workspace admission locator root must be absolute".to_owned());
        }
        let file = std::fs::File::open(path).map_err(|error| {
            format!(
                "workspace admission locator is unavailable at {}: {error}",
                path.display()
            )
        })?;
        let identity = catalog_file_identity(&file)?;
        if let Some(entries_by_root) = cached_catalog_entries(path, &identity)? {
            return resolve_catalog_entry(entries_by_root.as_ref(), project_root);
        }
        // SAFETY: catalog publication is immutable and atomic. Existing mappings retain
        // the previous inode while a writer publishes the next complete catalog.
        let mapping = unsafe { memmap2::MmapOptions::new().map(&file) }
            .map_err(|error| format!("map workspace admission locator: {error}"))?;
        let entries_by_root = Arc::new(
            decode_catalog(mapping.as_ref())?
                .into_iter()
                .map(|entry| (entry.project_root.clone(), entry))
                .collect(),
        );
        cache_catalog_entries(path, identity, Arc::clone(&entries_by_root))?;
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
) -> Result<RuntimeWorkspaceAdmissionCatalogEntry, String> {
    entries_by_root.get(project_root).cloned().ok_or_else(|| {
        format!(
            "canonical workspace scope is not admitted: projectRoot={}",
            project_root.display()
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
    for entry in document.entries {
        entry.validate()?;
        if entries
            .iter()
            .any(|existing: &RuntimeWorkspaceAdmissionCatalogEntry| {
                existing.project_root == entry.project_root
                    && existing.workspace_identity != entry.workspace_identity
            })
        {
            return Err(format!(
                "Runtime Server workspace admission catalog contains a root identity conflict: projectRoot={}",
                entry.project_root.display()
            ));
        }
        if !entries.insert(entry) {
            return Err(
                "Runtime Server workspace admission catalog contains duplicates".to_owned(),
            );
        }
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
    file.sync_all()
        .await
        .map_err(|error| format!("sync workspace admission catalog: {error}"))?;
    drop(file);
    tokio::fs::rename(&pending, path)
        .await
        .map_err(|error| format!("publish workspace admission catalog: {error}"))
}

#[cfg(test)]
#[path = "../tests/unit/runtime_server_admission_catalog.rs"]
mod tests;
