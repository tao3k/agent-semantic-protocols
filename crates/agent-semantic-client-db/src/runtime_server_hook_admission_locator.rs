//! Memory-mapped, lifecycle-published Hook admission authority.

use crate::{
    runtime_server_admission_catalog::{
        RuntimeWorkspaceAdmissionCatalog, RuntimeWorkspaceAdmissionCatalogEntry,
    },
    runtime_server_control::RuntimeServerEndpoint,
    seqlock_json_memory::{SeqlockJsonMemoryReader, SeqlockJsonMemoryWriter},
    workspace_db_ipc::WorkspaceDbIpcSession,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, RwLock},
};

pub const RUNTIME_HOOK_ADMISSION_LOCATOR_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-hook-admission-locator.v1";
const LOCATOR_CAPACITY_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHookAdmissionEndpointBinding {
    pub owner_epoch: u64,
    pub transport_contract_digest: String,
    pub runtime_artifact_path: String,
    pub runtime_artifact_digest: String,
    pub binding_token: String,
    pub data_plane_socket_path: String,
    pub workspace_store_path: String,
}

impl RuntimeHookAdmissionEndpointBinding {
    fn from_endpoint(endpoint: &RuntimeServerEndpoint) -> Self {
        Self {
            owner_epoch: endpoint.owner_epoch,
            transport_contract_digest: endpoint.transport_contract_digest.clone(),
            runtime_artifact_path: endpoint.runtime_artifact_path.clone(),
            runtime_artifact_digest: endpoint.runtime_binary_identity.value().to_owned(),
            binding_token: endpoint.binding_token.clone(),
            data_plane_socket_path: endpoint.data_plane_socket_path.clone(),
            workspace_store_path: endpoint.workspace_store_path.clone(),
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.owner_epoch == 0 {
            return Err("Hook admission locator owner epoch must be positive".to_owned());
        }
        for (name, value) in [
            (
                "transportContractDigest",
                self.transport_contract_digest.as_str(),
            ),
            ("runtimeArtifactPath", self.runtime_artifact_path.as_str()),
            (
                "runtimeArtifactDigest",
                self.runtime_artifact_digest.as_str(),
            ),
            ("bindingToken", self.binding_token.as_str()),
            ("dataPlaneSocketPath", self.data_plane_socket_path.as_str()),
            ("workspaceStorePath", self.workspace_store_path.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("Hook admission locator {name} must be non-empty"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHookWorkspaceAdmission {
    pub workspace_identity: String,
    pub canonical_project_root: PathBuf,
    pub lookup_roots: BTreeSet<PathBuf>,
}

impl RuntimeHookWorkspaceAdmission {
    fn from_catalog_entry(entry: &RuntimeWorkspaceAdmissionCatalogEntry) -> Self {
        Self {
            workspace_identity: entry.workspace_identity.clone(),
            canonical_project_root: entry.project_root.clone(),
            lookup_roots: BTreeSet::from([entry.project_root.clone()]),
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.workspace_identity.trim().is_empty() {
            return Err("Hook admission locator workspace identity must be non-empty".to_owned());
        }
        if !self.canonical_project_root.is_absolute() {
            return Err(
                "Hook admission locator canonical project root must be absolute".to_owned(),
            );
        }
        if self.lookup_roots.is_empty() || self.lookup_roots.iter().any(|root| !root.is_absolute())
        {
            return Err(
                "Hook admission locator lookup roots must be non-empty absolute paths".to_owned(),
            );
        }
        if !self.lookup_roots.contains(&self.canonical_project_root) {
            return Err(
                "Hook admission locator must include its canonical root as a lookup root"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHookAdmissionLocatorDocument {
    pub schema_id: String,
    pub schema_version: String,
    pub generation: u64,
    pub content_digest: String,
    pub endpoint: RuntimeHookAdmissionEndpointBinding,
    pub workspaces: Vec<RuntimeHookWorkspaceAdmission>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeHookAdmissionLocatorContent<'a> {
    endpoint: &'a RuntimeHookAdmissionEndpointBinding,
    workspaces: &'a [RuntimeHookWorkspaceAdmission],
}

impl RuntimeHookAdmissionLocatorDocument {
    fn new(
        generation: u64,
        endpoint: RuntimeHookAdmissionEndpointBinding,
        entries: &BTreeSet<RuntimeWorkspaceAdmissionCatalogEntry>,
    ) -> Result<Self, String> {
        let workspaces = entries
            .iter()
            .map(RuntimeHookWorkspaceAdmission::from_catalog_entry)
            .collect::<Vec<_>>();
        let content_digest = content_digest(&endpoint, &workspaces)?;
        let document = Self {
            schema_id: RUNTIME_HOOK_ADMISSION_LOCATOR_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            generation,
            content_digest,
            endpoint,
            workspaces,
        };
        document.validate()?;
        Ok(document)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != RUNTIME_HOOK_ADMISSION_LOCATOR_SCHEMA_ID || self.schema_version != "1"
        {
            return Err("Hook admission locator schema mismatch".to_owned());
        }
        if self.generation == 0 || self.generation % 2 != 0 {
            return Err("Hook admission locator generation must be positive and even".to_owned());
        }
        self.endpoint.validate()?;
        for workspace in &self.workspaces {
            workspace.validate()?;
        }
        let expected = content_digest(&self.endpoint, &self.workspaces)?;
        if self.content_digest != expected {
            return Err(format!(
                "Hook admission locator content digest mismatch: expected={expected} actual={}",
                self.content_digest
            ));
        }
        Ok(())
    }

    fn validate_runtime_lookup(&self) -> Result<(), String> {
        if self.schema_id != RUNTIME_HOOK_ADMISSION_LOCATOR_SCHEMA_ID || self.schema_version != "1"
        {
            return Err("Hook admission locator schema mismatch".to_owned());
        }
        if self.generation == 0 || self.generation % 2 != 0 {
            return Err("Hook admission locator generation must be positive and even".to_owned());
        }
        if !self.content_digest.starts_with("blake3-256:") || self.content_digest.len() != 75 {
            return Err("Hook admission locator content digest is malformed".to_owned());
        }
        self.endpoint.validate()
    }

    fn resolve(&self, lookup_root: &Path) -> Result<&RuntimeHookWorkspaceAdmission, String> {
        if !lookup_root.is_absolute() {
            return Err(format!(
                "Hook admission lookup root must be absolute: {}",
                lookup_root.display()
            ));
        }
        self.workspaces
            .iter()
            .find(|workspace| workspace.lookup_roots.contains(lookup_root))
            .ok_or_else(|| {
                format!(
                    "Runtime Server has not admitted Hook lookup root: {}",
                    lookup_root.display()
                )
            })
    }
}

fn content_digest(
    endpoint: &RuntimeHookAdmissionEndpointBinding,
    workspaces: &[RuntimeHookWorkspaceAdmission],
) -> Result<String, String> {
    let bytes = serde_json::to_vec(&RuntimeHookAdmissionLocatorContent {
        endpoint,
        workspaces,
    })
    .map_err(|error| format!("encode Hook admission locator digest payload: {error}"))?;
    Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
}

pub fn runtime_hook_admission_locator_path(state_home: &Path) -> PathBuf {
    state_home
        .join("runtime")
        .join("server")
        .join("hook-admission-locator.v1.memory")
}

pub struct RuntimeHookAdmissionLocatorAuthority {
    writer: SeqlockJsonMemoryWriter,
    endpoint: RuntimeHookAdmissionEndpointBinding,
}

impl RuntimeHookAdmissionLocatorAuthority {
    pub async fn start(
        state_home: &Path,
        endpoint: &RuntimeServerEndpoint,
    ) -> Result<Self, String> {
        endpoint.validate()?;
        let writer = SeqlockJsonMemoryWriter::create(
            &runtime_hook_admission_locator_path(state_home),
            LOCATOR_CAPACITY_BYTES,
        )
        .await?;
        Ok(Self {
            writer,
            endpoint: RuntimeHookAdmissionEndpointBinding::from_endpoint(endpoint),
        })
    }

    pub fn publish(
        &mut self,
        entries: &BTreeSet<RuntimeWorkspaceAdmissionCatalogEntry>,
    ) -> Result<u64, String> {
        let next_generation = self.writer.next_committed_generation();
        let document = RuntimeHookAdmissionLocatorDocument::new(
            next_generation,
            self.endpoint.clone(),
            entries,
        )?;
        self.writer.publish_postcard(&document)
    }
}

struct RuntimeHookAdmissionLocatorReader {
    mapping: SeqlockJsonMemoryReader,
    document: RwLock<Option<(u64, Arc<RuntimeHookAdmissionLocatorDocument>)>>,
}

impl RuntimeHookAdmissionLocatorReader {
    async fn open(path: &Path) -> Result<Self, String> {
        Ok(Self {
            mapping: SeqlockJsonMemoryReader::open(path).await?,
            document: RwLock::new(None),
        })
    }

    fn read_document(&self) -> Result<(u64, Arc<RuntimeHookAdmissionLocatorDocument>), String> {
        if let Some(generation) = self.mapping.stable_generation()
            && let Some((cached_generation, document)) = self
                .document
                .read()
                .map_err(|_| "Hook admission locator document cache is poisoned".to_owned())?
                .as_ref()
            && *cached_generation == generation
        {
            return Ok((generation, Arc::clone(document)));
        }
        let (generation, document) = self
            .mapping
            .read_postcard::<RuntimeHookAdmissionLocatorDocument>()?;
        let mut cached = self
            .document
            .write()
            .map_err(|_| "Hook admission locator document cache is poisoned".to_owned())?;
        if let Some((cached_generation, document)) = cached.as_ref()
            && *cached_generation == generation
        {
            return Ok((generation, Arc::clone(document)));
        }
        let document = Arc::new(document);
        *cached = Some((generation, Arc::clone(&document)));
        Ok((generation, document))
    }
}

static LOCATOR_READERS: OnceLock<
    dashmap::DashMap<PathBuf, Arc<RuntimeHookAdmissionLocatorReader>>,
> = OnceLock::new();

async fn locator_reader(
    path: &Path,
) -> Result<(Arc<RuntimeHookAdmissionLocatorReader>, bool), String> {
    let readers = LOCATOR_READERS.get_or_init(Default::default);
    if let Some(reader) = readers.get(path) {
        return Ok((Arc::clone(reader.value()), false));
    }
    let candidate = Arc::new(RuntimeHookAdmissionLocatorReader::open(path).await?);
    match readers.entry(path.to_path_buf()) {
        dashmap::mapref::entry::Entry::Occupied(reader) => Ok((Arc::clone(reader.get()), false)),
        dashmap::mapref::entry::Entry::Vacant(entry) => {
            entry.insert(Arc::clone(&candidate));
            Ok((candidate, true))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHookAdmissionLookupReceipt {
    pub git_discoveries: u64,
    pub package_scope_resolutions: u64,
    pub canonicalize_calls: u64,
    pub endpoint_json_reads: u64,
    pub locator_memory_opens: u64,
    pub locator_open_nanos: u64,
    pub document_read_nanos: u64,
    pub session_construct_nanos: u64,
}

fn read_and_connect_hook_workspace_session(
    reader: &RuntimeHookAdmissionLocatorReader,
    lookup_root: &Path,
) -> Result<(WorkspaceDbIpcSession, u64, u64), String> {
    let read_started = std::time::Instant::now();
    let (generation, document) = reader.read_document()?;
    document.validate_runtime_lookup()?;
    if generation != document.generation {
        return Err(format!(
            "Hook admission locator seqlock generation mismatch: memory={generation} document={}",
            document.generation
        ));
    }
    let workspace = document.resolve(lookup_root)?;
    workspace.validate()?;
    let document_read_nanos = read_started.elapsed().as_nanos() as u64;
    let session_started = std::time::Instant::now();
    let session = WorkspaceDbIpcSession::for_runtime_locator(
        &document.endpoint,
        workspace.workspace_identity.clone(),
        workspace.canonical_project_root.clone(),
    );
    Ok((
        session,
        document_read_nanos,
        session_started.elapsed().as_nanos() as u64,
    ))
}

pub async fn connect_hook_workspace_session(
    state_home: &Path,
    lookup_root: &Path,
) -> Result<WorkspaceDbIpcSession, String> {
    connect_hook_workspace_session_with_receipt(state_home, lookup_root)
        .await
        .map(|(session, _receipt)| session)
}

pub async fn connect_hook_workspace_session_with_receipt(
    state_home: &Path,
    lookup_root: &Path,
) -> Result<(WorkspaceDbIpcSession, RuntimeHookAdmissionLookupReceipt), String> {
    let open_started = std::time::Instant::now();
    let (reader, opened) = locator_reader(&runtime_hook_admission_locator_path(state_home)).await?;
    let locator_open_nanos = open_started.elapsed().as_nanos() as u64;
    let (session, document_read_nanos, session_construct_nanos) =
        read_and_connect_hook_workspace_session(&reader, lookup_root)?;
    Ok((
        session,
        RuntimeHookAdmissionLookupReceipt {
            git_discoveries: 0,
            package_scope_resolutions: 0,
            canonicalize_calls: 0,
            endpoint_json_reads: 0,
            locator_memory_opens: u64::from(opened),
            locator_open_nanos,
            document_read_nanos,
            session_construct_nanos,
        },
    ))
}

pub struct RuntimeHookAdmissionLocatorTask {
    shutdown: tokio::sync::watch::Sender<bool>,
    task: tokio::task::JoinHandle<Result<(), String>>,
}

impl RuntimeHookAdmissionLocatorTask {
    pub async fn shutdown(self) -> Result<(), String> {
        self.shutdown.send_replace(true);
        self.task
            .await
            .map_err(|error| format!("join Hook admission locator publisher: {error}"))?
    }
}

/// Publishes the current identity set and keeps it synchronized with later
/// admissions. The returned task is owned by the Runtime Server lifecycle.
pub async fn spawn_runtime_hook_admission_locator(
    state_home: PathBuf,
    endpoint: RuntimeServerEndpoint,
    catalog: RuntimeWorkspaceAdmissionCatalog,
) -> Result<RuntimeHookAdmissionLocatorTask, String> {
    let mut authority = RuntimeHookAdmissionLocatorAuthority::start(&state_home, &endpoint).await?;
    authority.publish(catalog.snapshot().as_ref())?;
    let mut admissions = catalog.subscribe();
    let (shutdown, mut shutdown_requested) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                changed = admissions.changed() => {
                    // Catalog closure is a normal terminal condition during
                    // Runtime Server shutdown: the admission owner is already
                    // being dropped and no further locator publication can be
                    // observed. Do not turn that cancellation race into a
                    // failed daemon drain receipt.
                    if changed.is_err() {
                        // The admission owner is dropped as part of the
                        // RuntimeServer shutdown join. Its watch closes before
                        // this auxiliary publisher observes the shutdown bit;
                        // that ordering is the normal terminal bridge.
                        return Ok(());
                    }
                    authority.publish(admissions.borrow_and_update().as_ref())?;
                }
                changed = shutdown_requested.changed() => {
                    changed.map_err(|_| {
                        "Runtime Server Hook admission locator shutdown channel closed".to_owned()
                    })?;
                    if *shutdown_requested.borrow() {
                        return Ok(());
                    }
                }
            }
        }
    });
    Ok(RuntimeHookAdmissionLocatorTask { shutdown, task })
}

#[cfg(test)]
#[path = "../tests/unit/runtime_server_hook_admission_locator.rs"]
mod tests;
