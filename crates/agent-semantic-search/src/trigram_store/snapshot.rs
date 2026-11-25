// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Deterministic, BLAKE3-verified zero-copy `mmap` binary snapshot codec
//! (`ASPTGRP1`) for [`crate::trigram_store::FlatTrigramStore`].

use std::fmt;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use memmap2::Mmap;
use rustc_hash::FxHashMap;

use super::{FlatTrigramStore, TrigramBacking, TrigramLookupEntry, TrigramPosting};

/// Binary format magic header (`ASPTGRP1`).
pub const SNAPSHOT_MAGIC: &[u8; 8] = b"ASPTGRP1";
/// Binary snapshot format version.
pub const SNAPSHOT_VERSION: u32 = 1;
/// Fixed header size in bytes (including 32-byte payload BLAKE3 digest).
pub const SNAPSHOT_HEADER_BYTES: usize = 80;
/// Fixed wire size of one sorted trigram lookup directory record.
pub const LOOKUP_RECORD_BYTES: usize = 16;
/// Fixed wire size of one compact trigram posting record.
pub const POSTING_RECORD_BYTES: usize = 6;

/// Raw file record stored inside the binary snapshot's files section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotFileRecord {
    pub relative_path: String,
    pub content_hash: [u8; 32],
    pub body: String,
}

/// Error returned when encoding, opening, or validating a binary trigram snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrigramSnapshotError {
    Io(String),
    Truncated(String),
    InvalidMagic,
    UnsupportedVersion(u32),
    ChecksumMismatch,
    CorruptLayout(String),
}

impl fmt::Display for TrigramSnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "snapshot I/O error: {msg}"),
            Self::Truncated(msg) => write!(f, "snapshot truncated: {msg}"),
            Self::InvalidMagic => write!(f, "snapshot magic header mismatch"),
            Self::UnsupportedVersion(ver) => write!(f, "unsupported snapshot version {ver}"),
            Self::ChecksumMismatch => write!(f, "snapshot BLAKE3 checksum mismatch"),
            Self::CorruptLayout(msg) => write!(f, "corrupt snapshot layout: {msg}"),
        }
    }
}

impl std::error::Error for TrigramSnapshotError {}

pub(crate) fn encode_snapshot_bytes(
    store: &FlatTrigramStore,
    live_files: &[SnapshotFileRecord],
) -> Vec<u8> {
    debug_assert!(store.delta.is_empty(), "compact before encoding snapshot");

    let lookup_count = store.unique_trigram_count();
    let posting_count = store.base_posting_count();

    let files_bytes_len: usize = live_files
        .iter()
        .map(|f| 4 + f.relative_path.len() + 4 + f.body.len() + 32)
        .sum();
    let lookup_bytes_len = lookup_count * LOOKUP_RECORD_BYTES;
    let postings_bytes_len = posting_count * POSTING_RECORD_BYTES;

    let files_offset = SNAPSHOT_HEADER_BYTES as u64;
    let lookup_offset = files_offset + files_bytes_len as u64;
    let postings_offset = lookup_offset + lookup_bytes_len as u64;

    let total_len = SNAPSHOT_HEADER_BYTES + files_bytes_len + lookup_bytes_len + postings_bytes_len;
    let mut buf = Vec::with_capacity(total_len);
    buf.resize(SNAPSHOT_HEADER_BYTES, 0);

    for file in live_files {
        let path_bytes = file.relative_path.as_bytes();
        let body_bytes = file.body.as_bytes();
        buf.extend_from_slice(
            &u32::try_from(path_bytes.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        buf.extend_from_slice(path_bytes);
        buf.extend_from_slice(
            &u32::try_from(body_bytes.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        buf.extend_from_slice(body_bytes);
        buf.extend_from_slice(&file.content_hash);
    }

    match &store.base {
        TrigramBacking::Owned { lookup, postings } => {
            for entry in lookup {
                buf.extend_from_slice(&entry.encode());
            }
            for posting in postings {
                buf.extend_from_slice(&posting.encode());
            }
        }
        TrigramBacking::Mapped {
            mmap,
            lookup_offset: l_off,
            lookup_count: l_cnt,
            postings_offset: p_off,
            postings_count: p_cnt,
        } => {
            buf.extend_from_slice(&mmap[*l_off..*l_off + (*l_cnt) * LOOKUP_RECORD_BYTES]);
            buf.extend_from_slice(&mmap[*p_off..*p_off + (*p_cnt) * POSTING_RECORD_BYTES]);
        }
    }

    let payload_hash = *blake3::hash(&buf[SNAPSHOT_HEADER_BYTES..]).as_bytes();
    buf[0..8].copy_from_slice(SNAPSHOT_MAGIC);
    buf[8..12].copy_from_slice(&SNAPSHOT_VERSION.to_le_bytes());
    buf[12..16].copy_from_slice(
        &u32::try_from(live_files.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    buf[16..20].copy_from_slice(
        &u32::try_from(lookup_count)
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    buf[20..24].copy_from_slice(
        &u32::try_from(posting_count)
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    buf[24..32].copy_from_slice(&files_offset.to_le_bytes());
    buf[32..40].copy_from_slice(&lookup_offset.to_le_bytes());
    buf[40..48].copy_from_slice(&postings_offset.to_le_bytes());
    buf[48..80].copy_from_slice(&payload_hash);
    buf
}

pub(crate) fn save_snapshot(
    store: &FlatTrigramStore,
    path: &Path,
    live_files: &[SnapshotFileRecord],
) -> Result<(), TrigramSnapshotError> {
    let bytes = encode_snapshot_bytes(store, live_files);
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| TrigramSnapshotError::Io(e.to_string()))?;
    }
    let mut file = File::create(path).map_err(|e| TrigramSnapshotError::Io(e.to_string()))?;
    file.write_all(&bytes)
        .map_err(|e| TrigramSnapshotError::Io(e.to_string()))?;
    file.sync_all()
        .map_err(|e| TrigramSnapshotError::Io(e.to_string()))?;
    Ok(())
}

pub(crate) fn load_mmap_snapshot(
    path: &Path,
) -> Result<(Vec<SnapshotFileRecord>, FlatTrigramStore), TrigramSnapshotError> {
    let file = File::open(path).map_err(|e| TrigramSnapshotError::Io(e.to_string()))?;
    // SAFETY: The snapshot file is opened read-only and verified via BLAKE3 + bounds checks
    // before any section is interpreted.
    let mmap = unsafe { Mmap::map(&file) }.map_err(|e| TrigramSnapshotError::Io(e.to_string()))?;
    let parsed = validate_and_parse_snapshot_header(&mmap)?;
    let files = decode_snapshot_files(
        &mmap,
        parsed.files_offset,
        parsed.lookup_offset,
        parsed.file_count,
    )?;
    let store = FlatTrigramStore {
        base: TrigramBacking::Mapped {
            mmap,
            lookup_offset: parsed.lookup_offset,
            lookup_count: parsed.lookup_count,
            postings_offset: parsed.postings_offset,
            postings_count: parsed.posting_count,
        },
        delta: FxHashMap::default(),
    };
    Ok((files, store))
}

pub(crate) fn decode_snapshot_bytes(
    bytes: &[u8],
) -> Result<(Vec<SnapshotFileRecord>, FlatTrigramStore), TrigramSnapshotError> {
    let parsed = validate_and_parse_snapshot_header(bytes)?;
    let files = decode_snapshot_files(
        bytes,
        parsed.files_offset,
        parsed.lookup_offset,
        parsed.file_count,
    )?;
    let mut lookup = Vec::with_capacity(parsed.lookup_count);
    for i in 0..parsed.lookup_count {
        let off = parsed.lookup_offset + i * LOOKUP_RECORD_BYTES;
        lookup.push(TrigramLookupEntry::decode(
            &bytes[off..off + LOOKUP_RECORD_BYTES],
        ));
    }
    let mut postings = Vec::with_capacity(parsed.posting_count);
    for i in 0..parsed.posting_count {
        let off = parsed.postings_offset + i * POSTING_RECORD_BYTES;
        postings.push(TrigramPosting::decode(
            &bytes[off..off + POSTING_RECORD_BYTES],
        ));
    }
    let store = FlatTrigramStore {
        base: TrigramBacking::Owned { lookup, postings },
        delta: FxHashMap::default(),
    };
    Ok((files, store))
}

struct ParsedSnapshotHeader {
    file_count: usize,
    lookup_count: usize,
    posting_count: usize,
    files_offset: usize,
    lookup_offset: usize,
    postings_offset: usize,
}

fn validate_and_parse_snapshot_header(
    bytes: &[u8],
) -> Result<ParsedSnapshotHeader, TrigramSnapshotError> {
    if bytes.len() < SNAPSHOT_HEADER_BYTES {
        return Err(TrigramSnapshotError::Truncated(format!(
            "expected at least {SNAPSHOT_HEADER_BYTES} bytes, got {}",
            bytes.len()
        )));
    }
    if &bytes[0..8] != SNAPSHOT_MAGIC {
        return Err(TrigramSnapshotError::InvalidMagic);
    }
    let version = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    if version != SNAPSHOT_VERSION {
        return Err(TrigramSnapshotError::UnsupportedVersion(version));
    }
    let expected_hash = &bytes[48..80];
    let actual_hash = blake3::hash(&bytes[SNAPSHOT_HEADER_BYTES..]);
    if actual_hash.as_bytes() != expected_hash {
        return Err(TrigramSnapshotError::ChecksumMismatch);
    }

    let file_count = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    let lookup_count = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
    let posting_count = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]) as usize;

    let files_offset = parse_u64_offset(&bytes[24..32], "files_offset")?;
    let lookup_offset = parse_u64_offset(&bytes[32..40], "lookup_offset")?;
    let postings_offset = parse_u64_offset(&bytes[40..48], "postings_offset")?;

    if files_offset != SNAPSHOT_HEADER_BYTES
        || lookup_offset < files_offset
        || postings_offset < lookup_offset
        || postings_offset > bytes.len()
    {
        return Err(TrigramSnapshotError::CorruptLayout(
            "invalid section offsets".to_owned(),
        ));
    }

    let lookup_bytes = lookup_count
        .checked_mul(LOOKUP_RECORD_BYTES)
        .ok_or_else(|| TrigramSnapshotError::CorruptLayout("lookup size overflow".to_owned()))?;
    let postings_bytes = posting_count
        .checked_mul(POSTING_RECORD_BYTES)
        .ok_or_else(|| TrigramSnapshotError::CorruptLayout("postings size overflow".to_owned()))?;

    if lookup_offset.checked_add(lookup_bytes) != Some(postings_offset) {
        return Err(TrigramSnapshotError::CorruptLayout(
            "lookup table bounds mismatch".to_owned(),
        ));
    }
    if postings_offset.checked_add(postings_bytes) != Some(bytes.len()) {
        return Err(TrigramSnapshotError::CorruptLayout(
            "postings table bounds mismatch".to_owned(),
        ));
    }

    verify_lookup_table_integrity(bytes, lookup_offset, lookup_count, posting_count)?;

    Ok(ParsedSnapshotHeader {
        file_count,
        lookup_count,
        posting_count,
        files_offset,
        lookup_offset,
        postings_offset,
    })
}

fn parse_u64_offset(slice: &[u8], name: &str) -> Result<usize, TrigramSnapshotError> {
    let raw = u64::from_le_bytes(slice.try_into().expect("8-byte offset slice"));
    usize::try_from(raw)
        .map_err(|_| TrigramSnapshotError::CorruptLayout(format!("{name} overflow")))
}

fn verify_lookup_table_integrity(
    bytes: &[u8],
    lookup_offset: usize,
    lookup_count: usize,
    posting_count: usize,
) -> Result<(), TrigramSnapshotError> {
    let mut prev_tri: Option<u32> = None;
    for i in 0..lookup_count {
        let off = lookup_offset + i * LOOKUP_RECORD_BYTES;
        let entry = TrigramLookupEntry::decode(&bytes[off..off + LOOKUP_RECORD_BYTES]);
        if let Some(prev) = prev_tri
            && entry.trigram <= prev
        {
            return Err(TrigramSnapshotError::CorruptLayout(
                "lookup table is not strictly sorted".to_owned(),
            ));
        }
        prev_tri = Some(entry.trigram);
        let end = entry
            .offset
            .checked_add(u64::from(entry.len))
            .ok_or_else(|| {
                TrigramSnapshotError::CorruptLayout("posting range overflow".to_owned())
            })?;
        if end > posting_count as u64 {
            return Err(TrigramSnapshotError::CorruptLayout(
                "lookup entry points past postings arena".to_owned(),
            ));
        }
    }
    Ok(())
}

fn decode_snapshot_files(
    bytes: &[u8],
    files_offset: usize,
    files_end: usize,
    file_count: usize,
) -> Result<Vec<SnapshotFileRecord>, TrigramSnapshotError> {
    let mut cursor = files_offset;
    let mut out = Vec::with_capacity(file_count.min(65_536));
    for _ in 0..file_count {
        if cursor + 4 > files_end {
            return Err(TrigramSnapshotError::Truncated("file path_len".to_owned()));
        }
        let path_len =
            u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().expect("4 bytes")) as usize;
        cursor += 4;
        if cursor + path_len + 4 > files_end {
            return Err(TrigramSnapshotError::Truncated(
                "file path bytes".to_owned(),
            ));
        }
        let relative_path = std::str::from_utf8(&bytes[cursor..cursor + path_len])
            .map_err(|_| TrigramSnapshotError::CorruptLayout("non-UTF8 relative_path".to_owned()))?
            .to_owned();
        cursor += path_len;

        let body_len =
            u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().expect("4 bytes")) as usize;
        cursor += 4;
        if cursor + body_len + 32 > files_end {
            return Err(TrigramSnapshotError::Truncated(
                "file body bytes".to_owned(),
            ));
        }
        let body = std::str::from_utf8(&bytes[cursor..cursor + body_len])
            .map_err(|_| TrigramSnapshotError::CorruptLayout("non-UTF8 file body".to_owned()))?
            .to_owned();
        cursor += body_len;

        let mut content_hash = [0u8; 32];
        content_hash.copy_from_slice(&bytes[cursor..cursor + 32]);
        cursor += 32;

        out.push(SnapshotFileRecord {
            relative_path,
            content_hash,
            body,
        });
    }
    if cursor != files_end {
        return Err(TrigramSnapshotError::CorruptLayout(
            "trailing bytes in files section".to_owned(),
        ));
    }
    Ok(out)
}
