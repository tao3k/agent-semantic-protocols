// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Bounded file-backed Hook inbox. The write boundary is published last.

use agent_semantic_client_protocol::{
    HookMemoryInboxEvent, HookMemoryInboxFailure, HookMemoryInboxFailureReason,
    HookWorkspaceMutationEvent,
};
use fs2::FileExt as _;
use memmap2::{Mmap, MmapOptions};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 8] = b"ASPIBX01";
const HEADER_LEN: usize = 32;
const CHECKSUM_LEN: usize = 32;
const LENGTH_LEN: usize = 4;
/// Fixed V1 capacity. Runtime compacts acknowledged prefixes in the background.
pub const HOOK_MEMORY_INBOX_BYTES: usize = 4 * 1024 * 1024;

/// One process-resident reader mapping. Runtime opens it once and probes only
/// the published header boundary on subsequent reconcile ticks.
pub struct HookMemoryInboxReader {
    file: File,
    mapping: Mmap,
}

impl HookMemoryInboxReader {
    pub fn open_or_create(path: &Path) -> Result<Self, HookMemoryInboxFailure> {
        let file = open(path)?;
        file.try_lock_exclusive().map_err(|error| {
            failure(
                HookMemoryInboxFailureReason::LockBudgetExceeded,
                format!("acquire Hook inbox initialization lock: {error}"),
            )
        })?;
        let initialized = initialize_file_mapping(&file);
        let unlock = file.unlock().map_err(|error| {
            failure(
                HookMemoryInboxFailureReason::Unlock,
                format!("release Hook inbox initialization lock: {error}"),
            )
        });
        match (initialized, unlock) {
            (Ok(()), Ok(())) => {}
            (Err(error), _) | (_, Err(error)) => return Err(error),
        }
        let mapping = unsafe { MmapOptions::new().map(&file) }
            .map_err(|error| failure(HookMemoryInboxFailureReason::Mmap, error.to_string()))?;
        Ok(Self { file, mapping })
    }

    pub fn read_after(
        &self,
        acknowledged_sequence: u64,
    ) -> Result<Vec<HookMemoryInboxEvent>, HookMemoryInboxFailure> {
        self.file.try_lock_shared().map_err(|error| {
            failure(
                HookMemoryInboxFailureReason::LockBudgetExceeded,
                format!("acquire Hook inbox resident reader lock: {error}"),
            )
        })?;
        let result = read_mapping(&self.mapping, acknowledged_sequence);
        let unlock = self.file.unlock().map_err(|error| {
            failure(
                HookMemoryInboxFailureReason::Unlock,
                format!("release Hook inbox resident reader lock: {error}"),
            )
        });
        match (result, unlock) {
            (Ok(events), Ok(())) => Ok(events),
            (Err(error), _) | (_, Err(error)) => Err(error),
        }
    }

    /// Reads only the V1 published boundary; no record bytes are decoded.
    pub fn latest_published_sequence(&self) -> Result<u64, HookMemoryInboxFailure> {
        self.file.try_lock_shared().map_err(|error| {
            failure(
                HookMemoryInboxFailureReason::LockBudgetExceeded,
                format!("acquire Hook inbox boundary reader lock: {error}"),
            )
        })?;
        let result = decode_header(&self.mapping).map(|(_, next)| next - 1);
        let unlock = self.file.unlock().map_err(|error| {
            failure(
                HookMemoryInboxFailureReason::Unlock,
                format!("release Hook inbox boundary reader lock: {error}"),
            )
        });
        match (result, unlock) {
            (Ok(sequence), Ok(())) => Ok(sequence),
            (Err(error), _) | (_, Err(error)) => Err(error),
        }
    }
}

/// Resolves the one State Home serving-mailbox path.
pub fn default_path() -> Result<PathBuf, String> {
    Ok(
        agent_semantic_artifacts::StateHomeLayout::from_process_environment()?
            .runtime_state()
            .serving()
            .hook_memory_inbox(),
    )
}

/// Appends one workspace mutation without Runtime IPC, process launch, or fsync.
pub fn append_workspace_mutation(
    path: &Path,
    event: HookWorkspaceMutationEvent,
) -> Result<u64, HookMemoryInboxFailure> {
    event
        .validate()
        .map_err(|error| failure(HookMemoryInboxFailureReason::EncodeEvent, error))?;
    let file = open(path)?;
    file.try_lock_exclusive().map_err(|error| {
        failure(
            HookMemoryInboxFailureReason::LockBudgetExceeded,
            format!("acquire Hook inbox writer lock: {error}"),
        )
    })?;
    let result = append_locked(&file, event);
    let unlock = file.unlock().map_err(|error| {
        failure(
            HookMemoryInboxFailureReason::Unlock,
            format!("release Hook inbox writer lock: {error}"),
        )
    });
    match (result, unlock) {
        (Ok(sequence), Ok(())) => Ok(sequence),
        (Err(error), _) | (_, Err(error)) => Err(error),
    }
}

/// Opens a short-lived mapping and decodes records after an acknowledged sequence.
pub fn read_after(
    path: &Path,
    acknowledged_sequence: u64,
) -> Result<Vec<HookMemoryInboxEvent>, HookMemoryInboxFailure> {
    let file = OpenOptions::new().read(true).open(path).map_err(|error| {
        failure(
            HookMemoryInboxFailureReason::Open,
            format!("open Hook inbox reader {}: {error}", path.display()),
        )
    })?;
    file.try_lock_shared().map_err(|error| {
        failure(
            HookMemoryInboxFailureReason::LockBudgetExceeded,
            format!("acquire Hook inbox reader lock: {error}"),
        )
    })?;
    let result = read_locked(&file, acknowledged_sequence);
    let unlock = file.unlock().map_err(|error| {
        failure(
            HookMemoryInboxFailureReason::Unlock,
            format!("release Hook inbox reader lock: {error}"),
        )
    });
    match (result, unlock) {
        (Ok(events), Ok(())) => Ok(events),
        (Err(error), _) | (_, Err(error)) => Err(error),
    }
}

/// Reclaims only the acknowledged prefix while preserving sequence identity.
/// Reclaims an acknowledged byte prefix while preserving the next sequence.
pub fn compact_through(
    path: &Path,
    acknowledged_sequence: u64,
) -> Result<usize, HookMemoryInboxFailure> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| {
            failure(
                HookMemoryInboxFailureReason::Open,
                format!("open Hook inbox compactor {}: {error}", path.display()),
            )
        })?;
    file.try_lock_exclusive().map_err(|error| {
        failure(
            HookMemoryInboxFailureReason::LockBudgetExceeded,
            format!("acquire Hook inbox compaction lock: {error}"),
        )
    })?;
    let result = compact_locked(&file, acknowledged_sequence);
    let unlock = file.unlock().map_err(|error| {
        failure(
            HookMemoryInboxFailureReason::Unlock,
            format!("release Hook inbox compaction lock: {error}"),
        )
    });
    match (result, unlock) {
        (Ok(reclaimed), Ok(())) => Ok(reclaimed),
        (Err(error), _) | (_, Err(error)) => Err(error),
    }
}

fn open(path: &Path) -> Result<File, HookMemoryInboxFailure> {
    let parent = path.parent().ok_or_else(|| {
        failure(
            HookMemoryInboxFailureReason::Open,
            "Hook inbox path has no parent",
        )
    })?;
    std::fs::create_dir_all(parent).map_err(|error| {
        failure(
            HookMemoryInboxFailureReason::Open,
            format!("create Hook inbox directory {}: {error}", parent.display()),
        )
    })?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| {
            failure(
                HookMemoryInboxFailureReason::Open,
                format!("open Hook inbox {}: {error}", path.display()),
            )
        })?;
    let length = file
        .metadata()
        .map_err(|error| failure(HookMemoryInboxFailureReason::Stat, error.to_string()))?
        .len();
    if length == 0 {
        file.set_len(HOOK_MEMORY_INBOX_BYTES as u64)
            .map_err(|error| {
                failure(
                    HookMemoryInboxFailureReason::Preallocate,
                    format!("preallocate Hook inbox: {error}"),
                )
            })?;
    } else if length != HOOK_MEMORY_INBOX_BYTES as u64 {
        return Err(failure(
            HookMemoryInboxFailureReason::SizeMismatch,
            format!(
                "Hook inbox size mismatch: expected={} observed={length}",
                HOOK_MEMORY_INBOX_BYTES
            ),
        ));
    }
    Ok(file)
}

fn append_locked(
    file: &File,
    event: HookWorkspaceMutationEvent,
) -> Result<u64, HookMemoryInboxFailure> {
    let mut mapping = unsafe {
        MmapOptions::new()
            .len(HOOK_MEMORY_INBOX_BYTES)
            .map_mut(file)
    }
    .map_err(|error| failure(HookMemoryInboxFailureReason::Mmap, error.to_string()))?;
    let (write_offset, sequence) = initialize_or_decode_header(&mut mapping)?;
    let envelope = HookMemoryInboxEvent::workspace_mutation(sequence, event)
        .map_err(|error| failure(HookMemoryInboxFailureReason::EncodeEvent, error))?;
    let payload = serde_json::to_vec(&envelope).map_err(|error| {
        failure(
            HookMemoryInboxFailureReason::EncodeEvent,
            format!("encode Hook inbox event: {error}"),
        )
    })?;
    let payload_len: u32 = payload.len().try_into().map_err(|_| {
        failure(
            HookMemoryInboxFailureReason::RecordTooLarge,
            "Hook inbox event exceeds u32 record length",
        )
    })?;
    let record_len = LENGTH_LEN
        .checked_add(CHECKSUM_LEN)
        .and_then(|length| length.checked_add(payload.len()))
        .ok_or_else(|| {
            failure(
                HookMemoryInboxFailureReason::RecordSizeOverflow,
                "Hook inbox record size overflow",
            )
        })?;
    let next_offset = write_offset.checked_add(record_len).ok_or_else(|| {
        failure(
            HookMemoryInboxFailureReason::RecordSizeOverflow,
            "Hook inbox write offset overflow",
        )
    })?;
    if next_offset > mapping.len() {
        return Err(failure(
            HookMemoryInboxFailureReason::CapacityExhausted,
            "Hook inbox has no capacity for the next record",
        ));
    }
    let checksum = blake3::hash(&payload);
    mapping[write_offset..write_offset + LENGTH_LEN].copy_from_slice(&payload_len.to_le_bytes());
    mapping[write_offset + LENGTH_LEN..write_offset + LENGTH_LEN + CHECKSUM_LEN]
        .copy_from_slice(checksum.as_bytes());
    mapping[write_offset + LENGTH_LEN + CHECKSUM_LEN..next_offset].copy_from_slice(&payload);
    let next_sequence = sequence.checked_add(1).ok_or_else(|| {
        failure(
            HookMemoryInboxFailureReason::SequenceOverflow,
            "Hook inbox sequence overflow",
        )
    })?;
    mapping[16..24].copy_from_slice(&(next_offset as u64).to_le_bytes());
    mapping[24..32].copy_from_slice(&next_sequence.to_le_bytes());
    mapping.flush_async().map_err(|error| {
        failure(
            HookMemoryInboxFailureReason::FlushSchedule,
            format!("schedule Hook inbox page flush: {error}"),
        )
    })?;
    Ok(sequence)
}

fn initialize_file_mapping(file: &File) -> Result<(), HookMemoryInboxFailure> {
    let mut mapping = unsafe {
        MmapOptions::new()
            .len(HOOK_MEMORY_INBOX_BYTES)
            .map_mut(file)
    }
    .map_err(|error| failure(HookMemoryInboxFailureReason::Mmap, error.to_string()))?;
    initialize_or_decode_header(&mut mapping)?;
    mapping.flush_async().map_err(|error| {
        failure(
            HookMemoryInboxFailureReason::FlushSchedule,
            format!("schedule Hook inbox header flush: {error}"),
        )
    })
}

fn initialize_or_decode_header(mapping: &mut [u8]) -> Result<(usize, u64), HookMemoryInboxFailure> {
    if mapping.len() < HEADER_LEN {
        return Err(failure(
            HookMemoryInboxFailureReason::TruncatedHeader,
            "Hook inbox is shorter than its V1 header",
        ));
    }
    if mapping[..HEADER_LEN].iter().all(|byte| *byte == 0) {
        let mapping_len = mapping.len() as u64;
        mapping[..8].copy_from_slice(MAGIC);
        mapping[8..16].copy_from_slice(&mapping_len.to_le_bytes());
        mapping[16..24].copy_from_slice(&(HEADER_LEN as u64).to_le_bytes());
        mapping[24..32].copy_from_slice(&1_u64.to_le_bytes());
        return Ok((HEADER_LEN, 1));
    }
    decode_header(mapping)
}

fn decode_header(mapping: &[u8]) -> Result<(usize, u64), HookMemoryInboxFailure> {
    if mapping.len() < HEADER_LEN {
        return Err(failure(
            HookMemoryInboxFailureReason::TruncatedHeader,
            "truncated Hook inbox header",
        ));
    }
    if &mapping[..8] != MAGIC {
        return Err(failure(
            HookMemoryInboxFailureReason::MagicMismatch,
            "Hook inbox magic mismatch",
        ));
    }
    let declared_size = u64::from_le_bytes(mapping[8..16].try_into().map_err(|_| {
        failure(
            HookMemoryInboxFailureReason::HeaderDecode,
            "decode Hook inbox size",
        )
    })?);
    if declared_size != mapping.len() as u64 {
        return Err(failure(
            HookMemoryInboxFailureReason::SizeMismatch,
            "Hook inbox declared size mismatch",
        ));
    }
    let write_offset = u64::from_le_bytes(mapping[16..24].try_into().map_err(|_| {
        failure(
            HookMemoryInboxFailureReason::HeaderDecode,
            "decode Hook inbox write offset",
        )
    })?);
    let write_offset: usize = write_offset.try_into().map_err(|_| {
        failure(
            HookMemoryInboxFailureReason::HeaderRange,
            "Hook inbox write offset exceeds usize",
        )
    })?;
    if !(HEADER_LEN..=mapping.len()).contains(&write_offset) {
        return Err(failure(
            HookMemoryInboxFailureReason::WriteOffsetInvalid,
            "Hook inbox write offset is outside the mapping",
        ));
    }
    let next_sequence = u64::from_le_bytes(mapping[24..32].try_into().map_err(|_| {
        failure(
            HookMemoryInboxFailureReason::HeaderDecode,
            "decode Hook inbox sequence",
        )
    })?);
    if next_sequence == 0 {
        return Err(failure(
            HookMemoryInboxFailureReason::HeaderRange,
            "Hook inbox next sequence must be positive",
        ));
    }
    Ok((write_offset, next_sequence))
}

fn read_locked(
    file: &File,
    acknowledged_sequence: u64,
) -> Result<Vec<HookMemoryInboxEvent>, HookMemoryInboxFailure> {
    let mapping = unsafe { MmapOptions::new().map(file) }
        .map_err(|error| failure(HookMemoryInboxFailureReason::Mmap, error.to_string()))?;
    read_mapping(&mapping, acknowledged_sequence)
}

fn read_mapping(
    mapping: &[u8],
    acknowledged_sequence: u64,
) -> Result<Vec<HookMemoryInboxEvent>, HookMemoryInboxFailure> {
    let (write_offset, _) = decode_header(mapping)?;
    let mut cursor = HEADER_LEN;
    let mut events = Vec::new();
    while cursor < write_offset {
        let record_header_end = cursor
            .checked_add(LENGTH_LEN + CHECKSUM_LEN)
            .ok_or_else(|| {
                failure(
                    HookMemoryInboxFailureReason::RecordSizeOverflow,
                    "Hook inbox record header overflow",
                )
            })?;
        if record_header_end > write_offset {
            return Err(failure(
                HookMemoryInboxFailureReason::TruncatedRecord,
                "Hook inbox record header is truncated",
            ));
        }
        let payload_len = u32::from_le_bytes(
            mapping[cursor..cursor + LENGTH_LEN]
                .try_into()
                .map_err(|_| {
                    failure(
                        HookMemoryInboxFailureReason::DecodeRecord,
                        "decode Hook inbox record length",
                    )
                })?,
        ) as usize;
        let record_end = record_header_end.checked_add(payload_len).ok_or_else(|| {
            failure(
                HookMemoryInboxFailureReason::RecordSizeOverflow,
                "Hook inbox record end overflow",
            )
        })?;
        if record_end > write_offset {
            return Err(failure(
                HookMemoryInboxFailureReason::TruncatedRecord,
                "Hook inbox record payload is truncated",
            ));
        }
        let payload = &mapping[record_header_end..record_end];
        if mapping[cursor + LENGTH_LEN..record_header_end] != *blake3::hash(payload).as_bytes() {
            return Err(failure(
                HookMemoryInboxFailureReason::ChecksumMismatch,
                "Hook inbox record checksum mismatch",
            ));
        }
        let event: HookMemoryInboxEvent = serde_json::from_slice(payload).map_err(|error| {
            failure(
                HookMemoryInboxFailureReason::DecodeRecord,
                format!("decode Hook inbox event: {error}"),
            )
        })?;
        event
            .validate()
            .map_err(|error| failure(HookMemoryInboxFailureReason::DecodeRecord, error))?;
        if event.inbox_sequence > acknowledged_sequence {
            events.push(event);
        }
        cursor = record_end;
    }
    Ok(events)
}

fn compact_locked(
    file: &File,
    acknowledged_sequence: u64,
) -> Result<usize, HookMemoryInboxFailure> {
    let mut mapping = unsafe {
        MmapOptions::new()
            .len(HOOK_MEMORY_INBOX_BYTES)
            .map_mut(file)
    }
    .map_err(|error| failure(HookMemoryInboxFailureReason::Mmap, error.to_string()))?;
    let (write_offset, _) = decode_header(&mapping)?;
    let mut cursor = HEADER_LEN;
    let mut retain_start = HEADER_LEN;
    while cursor < write_offset {
        let record_header_end = cursor
            .checked_add(LENGTH_LEN + CHECKSUM_LEN)
            .ok_or_else(|| {
                failure(
                    HookMemoryInboxFailureReason::RecordSizeOverflow,
                    "Hook inbox compaction header overflow",
                )
            })?;
        if record_header_end > write_offset {
            return Err(failure(
                HookMemoryInboxFailureReason::TruncatedRecord,
                "Hook inbox compaction observed a truncated header",
            ));
        }
        let payload_len = u32::from_le_bytes(
            mapping[cursor..cursor + LENGTH_LEN]
                .try_into()
                .map_err(|_| {
                    failure(
                        HookMemoryInboxFailureReason::LengthDecode,
                        "decode Hook inbox compaction record length",
                    )
                })?,
        ) as usize;
        let record_end = record_header_end.checked_add(payload_len).ok_or_else(|| {
            failure(
                HookMemoryInboxFailureReason::RecordSizeOverflow,
                "Hook inbox compaction record end overflow",
            )
        })?;
        if record_end > write_offset {
            return Err(failure(
                HookMemoryInboxFailureReason::TruncatedRecord,
                "Hook inbox compaction observed a truncated payload",
            ));
        }
        let event: HookMemoryInboxEvent =
            serde_json::from_slice(&mapping[record_header_end..record_end]).map_err(|error| {
                failure(
                    HookMemoryInboxFailureReason::DecodeRecord,
                    format!("decode Hook inbox event during compaction: {error}"),
                )
            })?;
        if event.inbox_sequence <= acknowledged_sequence {
            retain_start = record_end;
        } else {
            break;
        }
        cursor = record_end;
    }
    if retain_start == HEADER_LEN {
        return Ok(0);
    }
    let retained_len = write_offset - retain_start;
    mapping.copy_within(retain_start..write_offset, HEADER_LEN);
    let next_write_offset = HEADER_LEN + retained_len;
    mapping[next_write_offset..write_offset].fill(0);
    mapping[16..24].copy_from_slice(&(next_write_offset as u64).to_le_bytes());
    mapping.flush_async().map_err(|error| {
        failure(
            HookMemoryInboxFailureReason::FlushSchedule,
            format!("schedule Hook inbox compaction flush: {error}"),
        )
    })?;
    Ok(retain_start - HEADER_LEN)
}

fn failure(
    reason: HookMemoryInboxFailureReason,
    detail: impl Into<String>,
) -> HookMemoryInboxFailure {
    HookMemoryInboxFailure::new(reason, detail)
}

#[cfg(test)]
#[path = "../tests/unit/hook_memory_inbox.rs"]
mod tests;
