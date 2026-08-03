use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use memmap2::MmapOptions;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, RwLock};

use super::atomic_snapshot_pointer::{AtomicSnapshotPointerReader, AtomicSnapshotPointerWriter};

const SCHEMA_ID: &str = "agent.semantic-protocols.runtime-owner-identity-journal";
const POINTER_FILE: &str = "owner-identity-journal.pointer";
const POINTER_CONTEXT: &str = "runtime owner identity journal";

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_owner_identity_journal.rs"]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RuntimeOwnerIdentityState {
    Present,
    Missing,
    Mutating,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeOwnerIdentityEntry {
    pub owner_path: String,
    pub state: RuntimeOwnerIdentityState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mutation_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeOwnerIdentityJournalSnapshot {
    schema_id: String,
    schema_version: String,
    workspace_identity: String,
    epoch: u64,
    base_generation_digest: String,
    source_mutation_id: String,
    source_mutation_digest: String,
    previous_epoch_readable: bool,
    entries: Vec<RuntimeOwnerIdentityEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeOwnerIdentityJournalPointer {
    epoch: u64,
    segment_name: String,
}

#[derive(Clone, Debug)]
pub(super) struct RuntimeOwnerIdentityJournalPublisher {
    directory: PathBuf,
    pointer: AtomicSnapshotPointerWriter,
    state: Arc<Mutex<Option<RuntimeOwnerIdentityJournalSnapshot>>>,
}

impl RuntimeOwnerIdentityJournalPublisher {
    pub async fn open(directory: PathBuf) -> Result<Self, String> {
        tokio::fs::create_dir_all(&directory)
            .await
            .map_err(|error| format!("create runtime owner identity directory: {error}"))?;
        let pointer_path = directory.join(POINTER_FILE);
        let pointer =
            AtomicSnapshotPointerWriter::open(pointer_path.clone(), POINTER_CONTEXT).await?;
        let reader = AtomicSnapshotPointerReader::<RuntimeOwnerIdentityJournalPointer>::open(
            &pointer_path,
            POINTER_CONTEXT,
        )
        .await?;
        let current_pointer = tokio::task::spawn_blocking(move || reader.read_optional())
            .await
            .map_err(|error| {
                format!("read runtime owner identity pointer task failed: {error}")
            })??;
        let current = match current_pointer {
            Some(pointer) => Some(read_segment(&directory, &pointer, None).await?),
            None => None,
        };
        Ok(Self {
            directory,
            pointer,
            state: Arc::new(Mutex::new(current)),
        })
    }

    pub async fn rebase(
        &self,
        workspace_identity: &str,
        base_generation_digest: &str,
    ) -> Result<(), String> {
        let mut state = self.state.lock().await;
        if state.as_ref().is_some_and(|snapshot| {
            snapshot.workspace_identity == workspace_identity
                && snapshot.base_generation_digest == base_generation_digest
        }) {
            return Ok(());
        }
        let epoch = state.as_ref().map_or(1, |snapshot| snapshot.epoch + 1);
        let snapshot = RuntimeOwnerIdentityJournalSnapshot {
            schema_id: SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.to_owned(),
            epoch,
            base_generation_digest: base_generation_digest.to_owned(),
            source_mutation_id: format!("generation:{base_generation_digest}"),
            source_mutation_digest: mutation_digest(&BTreeMap::new()),
            previous_epoch_readable: state.is_some(),
            entries: Vec::new(),
        };
        self.publish_snapshot(&snapshot).await?;
        *state = Some(snapshot);
        Ok(())
    }

    pub async fn publish_delta(
        &self,
        workspace_identity: &str,
        base_generation_digest: &str,
        source_mutation_id: &str,
        delta: Vec<RuntimeOwnerIdentityEntry>,
    ) -> Result<(), String> {
        let mut state = self.state.lock().await;
        let mut delta_entries = BTreeMap::new();
        for entry in delta {
            validate_entry(&entry)?;
            if delta_entries
                .insert(entry.owner_path.clone(), entry)
                .is_some()
            {
                return Err("runtime owner identity delta contains a duplicate owner".to_owned());
            }
        }
        let source_mutation_digest = mutation_digest(&delta_entries);
        if let Some(snapshot) = state.as_ref().filter(|snapshot| {
            snapshot.workspace_identity == workspace_identity
                && snapshot.base_generation_digest == base_generation_digest
                && snapshot.source_mutation_id == source_mutation_id
        }) {
            if snapshot.source_mutation_digest == source_mutation_digest {
                return Ok(());
            }
            return Err(
                "runtime owner identity mutation replay payload does not match committed epoch"
                    .to_owned(),
            );
        }
        let mut entries = state
            .as_ref()
            .filter(|snapshot| {
                snapshot.workspace_identity == workspace_identity
                    && snapshot.base_generation_digest == base_generation_digest
            })
            .map(|snapshot| {
                snapshot
                    .entries
                    .iter()
                    .cloned()
                    .map(|entry| (entry.owner_path.clone(), entry))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        for entry in delta_entries.into_values() {
            entries.insert(entry.owner_path.clone(), entry);
        }
        let epoch = state.as_ref().map_or(1, |snapshot| snapshot.epoch + 1);
        let snapshot = RuntimeOwnerIdentityJournalSnapshot {
            schema_id: SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.to_owned(),
            epoch,
            base_generation_digest: base_generation_digest.to_owned(),
            source_mutation_id: source_mutation_id.to_owned(),
            source_mutation_digest,
            previous_epoch_readable: state.is_some(),
            entries: entries.into_values().collect(),
        };
        validate_snapshot(&snapshot)?;
        self.publish_snapshot(&snapshot).await?;
        *state = Some(snapshot);
        Ok(())
    }

    async fn publish_snapshot(
        &self,
        snapshot: &RuntimeOwnerIdentityJournalSnapshot,
    ) -> Result<(), String> {
        validate_snapshot(snapshot)?;
        let segment_name = format!("owner-identity-{}.json", snapshot.epoch);
        let segment_path = self.directory.join(&segment_name);
        let segment_pending = self
            .directory
            .join(format!(".owner-identity-{}.pending", snapshot.epoch));
        let bytes = serde_json::to_vec(snapshot)
            .map_err(|error| format!("encode runtime owner identity journal: {error}"))?;
        let mut segment_file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&segment_pending)
            .await
            .map_err(|error| format!("open runtime owner identity segment: {error}"))?;
        segment_file
            .write_all(&bytes)
            .await
            .map_err(|error| format!("write runtime owner identity segment: {error}"))?;
        segment_file
            .sync_all()
            .await
            .map_err(|error| format!("sync runtime owner identity segment: {error}"))?;
        drop(segment_file);
        tokio::fs::rename(&segment_pending, &segment_path)
            .await
            .map_err(|error| format!("publish runtime owner identity segment: {error}"))?;
        tokio::fs::File::open(&self.directory)
            .await
            .map_err(|error| format!("open runtime owner identity directory: {error}"))?
            .sync_all()
            .await
            .map_err(|error| format!("sync runtime owner identity directory: {error}"))?;
        self.pointer
            .publish(&RuntimeOwnerIdentityJournalPointer {
                epoch: snapshot.epoch,
                segment_name,
            })
            .await
    }
}

#[derive(Debug)]
pub(super) struct RuntimeOwnerIdentityJournalReader {
    directory: PathBuf,
    pointer: AtomicSnapshotPointerReader<RuntimeOwnerIdentityJournalPointer>,
    snapshot: RwLock<Option<RuntimeOwnerIdentityJournalSnapshot>>,
    counters: Arc<RuntimeOwnerIdentityJournalCounters>,
}

#[derive(Debug, Default)]
struct RuntimeOwnerIdentityJournalCounters {
    segment_opens: AtomicU64,
    segment_decodes: AtomicU64,
}

impl RuntimeOwnerIdentityJournalReader {
    pub async fn open(generation_pointer_path: &Path) -> Result<Self, String> {
        let directory = generation_pointer_path
            .parent()
            .ok_or_else(|| "workspace generation pointer has no parent".to_owned())?
            .to_path_buf();
        let pointer =
            AtomicSnapshotPointerReader::open(&directory.join(POINTER_FILE), POINTER_CONTEXT)
                .await?;
        let counters = Arc::new(RuntimeOwnerIdentityJournalCounters::default());
        let snapshot = match pointer.read_optional()? {
            Some(current) => Some(read_segment(&directory, &current, Some(&counters)).await?),
            None => None,
        };
        Ok(Self {
            directory,
            pointer,
            snapshot: RwLock::new(snapshot),
            counters,
        })
    }

    pub async fn owner_is_current(
        &self,
        workspace_identity: &str,
        base_generation_digest: &str,
        owner_path: &str,
        owner_content_digest: &str,
    ) -> Result<bool, String> {
        self.refresh_if_changed().await?;
        let snapshot = self.snapshot.read().await;
        let Some(snapshot) = snapshot.as_ref() else {
            return Ok(false);
        };
        if snapshot.workspace_identity != workspace_identity
            || snapshot.base_generation_digest != base_generation_digest
        {
            return Ok(false);
        }
        Ok(
            match snapshot
                .entries
                .binary_search_by(|entry| entry.owner_path.as_str().cmp(owner_path))
            {
                Ok(index) => {
                    let entry = &snapshot.entries[index];
                    entry.state == RuntimeOwnerIdentityState::Present
                        && entry.content_digest.as_deref() == Some(owner_content_digest)
                }
                Err(_) => true,
            },
        )
    }

    async fn refresh_if_changed(&self) -> Result<(), String> {
        let Some(pointer) = self.pointer.read_optional()? else {
            return Ok(());
        };
        if self
            .snapshot
            .read()
            .await
            .as_ref()
            .is_some_and(|snapshot| snapshot.epoch == pointer.epoch)
        {
            return Ok(());
        }
        let next = read_segment(&self.directory, &pointer, Some(&self.counters)).await?;
        *self.snapshot.write().await = Some(next);
        Ok(())
    }

    #[cfg(test)]
    fn counter_snapshot(&self) -> (u64, u64, u64) {
        (
            self.counters.segment_opens.load(Ordering::Relaxed),
            self.counters.segment_decodes.load(Ordering::Relaxed),
            self.pointer.decode_count(),
        )
    }
}

async fn read_segment(
    directory: &Path,
    pointer: &RuntimeOwnerIdentityJournalPointer,
    counters: Option<&RuntimeOwnerIdentityJournalCounters>,
) -> Result<RuntimeOwnerIdentityJournalSnapshot, String> {
    validate_segment_name(&pointer.segment_name)?;
    if let Some(counters) = counters {
        counters.segment_opens.fetch_add(1, Ordering::Relaxed);
    }
    let snapshot =
        map_json::<RuntimeOwnerIdentityJournalSnapshot>(&directory.join(&pointer.segment_name))
            .await
            .map_err(|error| format!("open runtime owner identity segment: {error}"))?;
    if let Some(counters) = counters {
        counters.segment_decodes.fetch_add(1, Ordering::Relaxed);
    }
    if pointer.epoch != snapshot.epoch {
        return Err("runtime owner identity pointer epoch mismatch".to_owned());
    }
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

async fn map_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, std::io::Error> {
    let file = tokio::fs::File::open(path).await?.into_std().await;
    let mapping = unsafe {
        // SAFETY: segments are immutable after atomic publication and the
        // mapping is consumed before this function returns.
        MmapOptions::new().map(&file)?
    };
    serde_json::from_slice(&mapping)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn validate_segment_name(segment_name: &str) -> Result<(), String> {
    let mut components = Path::new(segment_name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err("runtime owner identity segment name must be one relative file".to_owned());
    }
    Ok(())
}

fn validate_snapshot(snapshot: &RuntimeOwnerIdentityJournalSnapshot) -> Result<(), String> {
    if snapshot.schema_id != SCHEMA_ID
        || snapshot.schema_version != "1"
        || snapshot.workspace_identity.trim().is_empty()
        || snapshot.epoch == 0
        || snapshot.source_mutation_id.trim().is_empty()
    {
        return Err("runtime owner identity journal identity is incomplete".to_owned());
    }
    validate_digest(&snapshot.base_generation_digest)?;
    validate_digest(&snapshot.source_mutation_digest)?;
    for pair in snapshot.entries.windows(2) {
        if pair[0].owner_path >= pair[1].owner_path {
            return Err(
                "runtime owner identity journal entries must be sorted and unique".to_owned(),
            );
        }
    }
    snapshot.entries.iter().try_for_each(validate_entry)
}

fn validate_entry(entry: &RuntimeOwnerIdentityEntry) -> Result<(), String> {
    super::owner_content_identity::normalized_owner_path(&entry.owner_path)?;
    match (
        &entry.state,
        entry.content_digest.as_deref(),
        entry.mutation_id.as_deref(),
    ) {
        (RuntimeOwnerIdentityState::Present, Some(digest), None) => validate_digest(digest),
        (RuntimeOwnerIdentityState::Missing, None, None) => Ok(()),
        (RuntimeOwnerIdentityState::Mutating, None, Some(mutation_id))
            if !mutation_id.trim().is_empty() =>
        {
            Ok(())
        }
        _ => Err("runtime owner identity entry state/digest mismatch".to_owned()),
    }
}

fn validate_digest(digest: &str) -> Result<(), String> {
    let Some(hex) = digest.strip_prefix("blake3-256:") else {
        return Err("runtime owner identity digest must use blake3-256".to_owned());
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("runtime owner identity digest must contain 64 hex characters".to_owned());
    }
    Ok(())
}

fn mutation_digest(entries: &BTreeMap<String, RuntimeOwnerIdentityEntry>) -> String {
    let mut hasher = blake3::Hasher::new();
    for (owner_path, entry) in entries {
        hasher.update(&(owner_path.len() as u64).to_le_bytes());
        hasher.update(owner_path.as_bytes());
        match &entry.state {
            RuntimeOwnerIdentityState::Present => hasher.update(&[1]),
            RuntimeOwnerIdentityState::Missing => hasher.update(&[0]),
            RuntimeOwnerIdentityState::Mutating => hasher.update(&[2]),
        };
        let digest = entry.content_digest.as_deref().unwrap_or_default();
        hasher.update(&(digest.len() as u64).to_le_bytes());
        hasher.update(digest.as_bytes());
        let mutation_id = entry.mutation_id.as_deref().unwrap_or_default();
        hasher.update(&(mutation_id.len() as u64).to_le_bytes());
        hasher.update(mutation_id.as_bytes());
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}
