//! Runtime-independent, file-backed mmap inbox for all one-shot Hook events.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_semantic_client_db::workspace_db_ipc::AgentHostLifecycleEventIpc;
use fs2::FileExt;
use memmap2::{MmapMut, MmapOptions};
use serde::{Deserialize, Serialize};

const INBOX_SCHEMA_ID: &str = "agent.semantic-protocols.hook.memory-inbox";
const INBOX_SCHEMA_VERSION: &str = "1";
const INBOX_FILE: &str = "hook-memory-inbox.v1.mmap";
const INBOX_LOCK_FILE: &str = "hook-memory-inbox.v1.mmap.lock";
const INBOX_MAGIC: &[u8; 16] = b"ASP-HOOK-INBX1\0\0";
const INBOX_BYTES: u64 = 4 * 1024 * 1024;
const HEADER_BYTES: usize = 4096;
const RECORD_HEADER_BYTES: usize = 4 + 32;
const WRITE_OFFSET_RANGE: std::ops::Range<usize> = 16..24;
const NEXT_SEQUENCE_RANGE: std::ops::Range<usize> = 24..32;
const ACK_SEQUENCE_RANGE: std::ops::Range<usize> = 32..40;
const INBOX_LOCK_BUDGET: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct HookMemoryInboxRecord {
    schema_id: String,
    schema_version: String,
    pub(crate) inbox_sequence: u64,
    pub(crate) entry_kind: String,
    pub(crate) event: serde_json::Value,
}

pub(crate) async fn append_host_lifecycle_event(
    project_root: &Path,
    event: AgentHostLifecycleEventIpc,
) -> Result<u64, String> {
    let state_dir = agent_semantic_runtime::ensure_project_hook_state_dir(project_root)?;
    append_host_lifecycle_event_in_state_dir(&state_dir, event).await
}

pub(crate) async fn append_host_lifecycle_event_in_state_dir(
    state_dir: &Path,
    mut event: AgentHostLifecycleEventIpc,
) -> Result<u64, String> {
    let identity = event.host_event_id.clone();
    append_record(state_dir, "host-lifecycle", &identity, |sequence| {
        event.host_event_sequence = sequence;
        serde_json::to_value(event)
            .map_err(|error| inbox_failure("encode-event", error.to_string()))
    })
    .await
}

pub(crate) async fn append_workspace_mutation(
    project_root: &Path,
    mutation_id: &str,
    changed_paths: Vec<String>,
) -> Result<u64, String> {
    let state_dir = agent_semantic_runtime::ensure_project_hook_state_dir(project_root)?;
    append_workspace_mutation_in_state_dir(&state_dir, mutation_id, changed_paths).await
}

pub(crate) async fn append_workspace_mutation_in_state_dir(
    state_dir: &Path,
    mutation_id: &str,
    changed_paths: Vec<String>,
) -> Result<u64, String> {
    append_record(state_dir, "workspace-mutation", mutation_id, |_| {
        Ok(serde_json::json!({
            "mutationId": mutation_id,
            "changedPaths": changed_paths,
        }))
    })
    .await
}

pub(crate) async fn append_runtime_performance_observation(
    project_root: &Path,
    observation: serde_json::Value,
) -> Result<u64, String> {
    let state_dir = agent_semantic_runtime::ensure_project_hook_state_dir(project_root)?;
    append_runtime_performance_observation_in_state_dir(&state_dir, observation).await
}

pub(crate) async fn append_runtime_performance_observation_in_state_dir(
    state_dir: &Path,
    observation: serde_json::Value,
) -> Result<u64, String> {
    let encoded = serde_json::to_vec(&observation)
        .map_err(|error| inbox_failure("encode-performance-observation", error.to_string()))?;
    let identity = format!("blake3-256:{}", blake3::hash(&encoded).to_hex());
    append_record(
        state_dir,
        "runtime-performance-observation",
        &identity,
        |_| Ok(observation),
    )
    .await
}

async fn append_record(
    state_dir: &Path,
    entry_kind: &str,
    identity: &str,
    materialize_event: impl FnOnce(u64) -> Result<serde_json::Value, String>,
) -> Result<u64, String> {
    fs_create_state_dir(state_dir)?;
    let lock = acquire_writer_lock(&state_dir).await?;
    let state_path = state_dir.join(INBOX_FILE);
    let file = open_inbox(&state_path)?;
    let mut mapped = map_inbox_mut(&file, &state_path)?;
    initialize_or_validate(&mut mapped, &state_path)?;
    let current = read_records(&mapped, &state_path)?;
    if let Some(existing) = current.iter().find(|record| {
        record.entry_kind == entry_kind && record_identity(record).as_deref() == Some(identity)
    }) {
        unlock(lock, &state_path)?;
        return Ok(existing.inbox_sequence);
    }

    let sequence = read_u64(&mapped, NEXT_SEQUENCE_RANGE)?;
    let next_sequence = sequence
        .checked_add(1)
        .ok_or_else(|| inbox_failure("sequence-overflow", state_path.display().to_string()))?;
    let record = HookMemoryInboxRecord {
        schema_id: INBOX_SCHEMA_ID.to_owned(),
        schema_version: INBOX_SCHEMA_VERSION.to_owned(),
        inbox_sequence: sequence,
        entry_kind: entry_kind.to_owned(),
        event: materialize_event(sequence)?,
    };
    let encoded = serde_json::to_vec(&record)
        .map_err(|error| inbox_failure("encode-record", error.to_string()))?;
    ensure_append_capacity(&mut mapped, &state_path, encoded.len())?;
    append_encoded_record(&mut mapped, &encoded, &state_path)?;
    write_u64(&mut mapped, NEXT_SEQUENCE_RANGE, next_sequence)?;
    mapped
        .flush_async()
        .map_err(|error| inbox_failure("flush-schedule", error.to_string()))?;
    unlock(lock, &state_path)?;
    Ok(sequence)
}

pub(crate) async fn pending_hook_events(
    project_root: &Path,
) -> Result<Vec<HookMemoryInboxRecord>, String> {
    let state_dir = agent_semantic_runtime::ensure_project_hook_state_dir(project_root)?;
    pending_hook_events_in_state_dir(&state_dir).await
}

pub(crate) async fn pending_hook_events_in_state_dir(
    state_dir: &Path,
) -> Result<Vec<HookMemoryInboxRecord>, String> {
    fs_create_state_dir(state_dir)?;
    let state_path = state_dir.join(INBOX_FILE);
    if !state_path.exists() {
        return Ok(Vec::new());
    }
    let lock = acquire_writer_lock(&state_dir).await?;
    let file = open_inbox(&state_path)?;
    let mut mapped = map_inbox_mut(&file, &state_path)?;
    initialize_or_validate(&mut mapped, &state_path)?;
    let acknowledged = read_u64(&mapped, ACK_SEQUENCE_RANGE)?;
    let records = read_records(&mapped, &state_path)?
        .into_iter()
        .filter(|record| record.inbox_sequence > acknowledged)
        .collect();
    unlock(lock, &state_path)?;
    Ok(records)
}

pub(crate) async fn acknowledge_hook_event(
    project_root: &Path,
    acknowledged_sequence: u64,
) -> Result<(), String> {
    let state_dir = agent_semantic_runtime::ensure_project_hook_state_dir(project_root)?;
    acknowledge_hook_event_in_state_dir(&state_dir, acknowledged_sequence).await
}

pub(crate) async fn acknowledge_hook_event_in_state_dir(
    state_dir: &Path,
    acknowledged_sequence: u64,
) -> Result<(), String> {
    fs_create_state_dir(state_dir)?;
    let lock = acquire_writer_lock(&state_dir).await?;
    let state_path = state_dir.join(INBOX_FILE);
    let file = open_inbox(&state_path)?;
    let mut mapped = map_inbox_mut(&file, &state_path)?;
    initialize_or_validate(&mut mapped, &state_path)?;
    let current = read_u64(&mapped, ACK_SEQUENCE_RANGE)?;
    if acknowledged_sequence > current {
        write_u64(&mut mapped, ACK_SEQUENCE_RANGE, acknowledged_sequence)?;
        mapped
            .flush_async()
            .map_err(|error| inbox_failure("flush-schedule", error.to_string()))?;
    }
    unlock(lock, &state_path)
}

fn fs_create_state_dir(state_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(state_dir).map_err(|error| {
        inbox_failure(
            "create-state-dir",
            format!("{}: {error}", state_dir.display()),
        )
    })
}

fn open_inbox(path: &Path) -> Result<File, String> {
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| inbox_failure("open", format!("{}: {error}", path.display())))?;
    let length = file
        .metadata()
        .map_err(|error| inbox_failure("stat", format!("{}: {error}", path.display())))?
        .len();
    if length == 0 {
        file.set_len(INBOX_BYTES).map_err(|error| {
            inbox_failure("preallocate", format!("{}: {error}", path.display()))
        })?;
    } else if length != INBOX_BYTES {
        return Err(inbox_failure(
            "size-mismatch",
            format!("{} expected={INBOX_BYTES} actual={length}", path.display()),
        ));
    }
    Ok(file)
}

fn map_inbox_mut(file: &File, path: &Path) -> Result<MmapMut, String> {
    // SAFETY: all writers and readers in this contract hold the same fs2 lock;
    // the file has a fixed length and is never truncated while mapped.
    unsafe { MmapOptions::new().map_mut(file) }
        .map_err(|error| inbox_failure("mmap", format!("{}: {error}", path.display())))
}

fn initialize_or_validate(mapped: &mut MmapMut, path: &Path) -> Result<(), String> {
    if mapped[..INBOX_MAGIC.len()].iter().all(|byte| *byte == 0) {
        mapped[..INBOX_MAGIC.len()].copy_from_slice(INBOX_MAGIC);
        write_u64(mapped, WRITE_OFFSET_RANGE, HEADER_BYTES as u64)?;
        write_u64(mapped, NEXT_SEQUENCE_RANGE, 1)?;
        write_u64(mapped, ACK_SEQUENCE_RANGE, 0)?;
        mapped
            .flush_async()
            .map_err(|error| inbox_failure("flush-schedule", error.to_string()))?;
        return Ok(());
    }
    if &mapped[..INBOX_MAGIC.len()] != INBOX_MAGIC {
        return Err(inbox_failure("magic-mismatch", path.display().to_string()));
    }
    let write_offset = read_u64(mapped, WRITE_OFFSET_RANGE)? as usize;
    if !(HEADER_BYTES..=mapped.len()).contains(&write_offset) {
        return Err(inbox_failure(
            "write-offset-invalid",
            format!("{} offset={write_offset}", path.display()),
        ));
    }
    Ok(())
}

fn ensure_append_capacity(
    mapped: &mut MmapMut,
    path: &Path,
    payload_len: usize,
) -> Result<(), String> {
    let required = RECORD_HEADER_BYTES
        .checked_add(payload_len)
        .ok_or_else(|| inbox_failure("record-size-overflow", payload_len.to_string()))?;
    let offset = read_u64(mapped, WRITE_OFFSET_RANGE)? as usize;
    if offset + required <= mapped.len() {
        return Ok(());
    }

    let acknowledged = read_u64(mapped, ACK_SEQUENCE_RANGE)?;
    let retained = read_records(mapped, path)?
        .into_iter()
        .filter(|record| record.inbox_sequence > acknowledged)
        .map(|record| {
            serde_json::to_vec(&record)
                .map_err(|error| inbox_failure("compact-encode", error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    mapped[HEADER_BYTES..].fill(0);
    write_u64(mapped, WRITE_OFFSET_RANGE, HEADER_BYTES as u64)?;
    for record in retained {
        append_encoded_record(mapped, &record, path)?;
    }
    let compacted_offset = read_u64(mapped, WRITE_OFFSET_RANGE)? as usize;
    if compacted_offset + required > mapped.len() {
        return Err(inbox_failure(
            "capacity-exhausted",
            format!("{} capacity={INBOX_BYTES}", path.display()),
        ));
    }
    Ok(())
}

fn append_encoded_record(mapped: &mut MmapMut, encoded: &[u8], path: &Path) -> Result<(), String> {
    let length = u32::try_from(encoded.len())
        .map_err(|_| inbox_failure("record-too-large", encoded.len().to_string()))?;
    let offset = read_u64(mapped, WRITE_OFFSET_RANGE)? as usize;
    let end = offset + RECORD_HEADER_BYTES + encoded.len();
    if end > mapped.len() {
        return Err(inbox_failure(
            "capacity-exhausted",
            path.display().to_string(),
        ));
    }
    mapped[offset..offset + 4].copy_from_slice(&length.to_le_bytes());
    let digest = blake3::hash(encoded);
    mapped[offset + 4..offset + RECORD_HEADER_BYTES].copy_from_slice(digest.as_bytes());
    mapped[offset + RECORD_HEADER_BYTES..end].copy_from_slice(encoded);
    // Publish the new boundary last while the cross-process writer lock is held.
    write_u64(mapped, WRITE_OFFSET_RANGE, end as u64)
}

fn read_records(mapped: &[u8], path: &Path) -> Result<Vec<HookMemoryInboxRecord>, String> {
    let write_offset = read_u64(mapped, WRITE_OFFSET_RANGE)? as usize;
    let mut cursor = HEADER_BYTES;
    let mut records = Vec::new();
    while cursor < write_offset {
        if cursor + RECORD_HEADER_BYTES > write_offset {
            return Err(inbox_failure(
                "truncated-header",
                path.display().to_string(),
            ));
        }
        let length = u32::from_le_bytes(
            mapped[cursor..cursor + 4]
                .try_into()
                .map_err(|_| inbox_failure("length-decode", cursor.to_string()))?,
        ) as usize;
        let payload_start = cursor + RECORD_HEADER_BYTES;
        let payload_end = payload_start
            .checked_add(length)
            .ok_or_else(|| inbox_failure("record-size-overflow", cursor.to_string()))?;
        if payload_end > write_offset {
            return Err(inbox_failure(
                "truncated-record",
                path.display().to_string(),
            ));
        }
        let payload = &mapped[payload_start..payload_end];
        if mapped[cursor + 4..payload_start] != *blake3::hash(payload).as_bytes() {
            return Err(inbox_failure("checksum-mismatch", cursor.to_string()));
        }
        records.push(
            serde_json::from_slice::<HookMemoryInboxRecord>(payload)
                .map_err(|error| inbox_failure("decode-record", error.to_string()))?,
        );
        cursor = payload_end;
    }
    records.sort_by_key(|record| record.inbox_sequence);
    Ok(records)
}

fn read_u64(mapped: &[u8], range: std::ops::Range<usize>) -> Result<u64, String> {
    mapped
        .get(range)
        .ok_or_else(|| inbox_failure("header-range", "u64".to_owned()))?
        .try_into()
        .map(u64::from_le_bytes)
        .map_err(|_| inbox_failure("header-decode", "u64".to_owned()))
}

fn write_u64(mapped: &mut [u8], range: std::ops::Range<usize>, value: u64) -> Result<(), String> {
    mapped
        .get_mut(range)
        .ok_or_else(|| inbox_failure("header-range", "u64".to_owned()))?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn record_identity(record: &HookMemoryInboxRecord) -> Option<String> {
    match record.entry_kind.as_str() {
        "host-lifecycle" => record
            .event
            .get("hostEventId")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        "host-execution-observation" => record
            .event
            .get("observationId")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        "workspace-mutation" => record
            .event
            .get("mutationId")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        "runtime-performance-observation" => serde_json::to_vec(&record.event)
            .ok()
            .map(|encoded| format!("blake3-256:{}", blake3::hash(&encoded).to_hex())),
        _ => None,
    }
}

async fn acquire_writer_lock(state_dir: &Path) -> Result<File, String> {
    let lock_path: PathBuf = state_dir.join(INBOX_LOCK_FILE);
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| inbox_failure("lock-open", format!("{}: {error}", lock_path.display())))?;
    let started = Instant::now();
    loop {
        match lock.try_lock_exclusive() {
            Ok(()) => return Ok(lock),
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    && started.elapsed() < INBOX_LOCK_BUDGET =>
            {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                return Err(inbox_failure(
                    "lock-budget-exceeded",
                    format!(
                        "{}ms {}",
                        INBOX_LOCK_BUDGET.as_millis(),
                        lock_path.display()
                    ),
                ));
            }
            Err(error) => {
                return Err(inbox_failure(
                    "lock-failed",
                    format!("{}: {error}", lock_path.display()),
                ));
            }
        }
    }
}

fn unlock(lock: File, path: &Path) -> Result<(), String> {
    FileExt::unlock(&lock)
        .map_err(|error| inbox_failure("unlock", format!("{}: {error}", path.display())))
}

fn inbox_failure(reason_kind: &str, detail: String) -> String {
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.hook.memory-inbox-failure",
        "schemaVersion": "1",
        "state": "failed",
        "reasonKind": reason_kind,
        "detail": detail,
        "retryAfterMs": 0,
    })
    .to_string()
}

#[cfg(test)]
#[path = "../../tests/unit/command/hook_runtime_memory_inbox.rs"]
mod tests;
