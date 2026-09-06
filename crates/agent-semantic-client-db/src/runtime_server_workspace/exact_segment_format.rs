// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Binary format boundary for immutable exact-projection mmap segments.

use super::evidence_context::CONTEXT_ENTRY_LEN;
use super::{
    BLOB_OFFSET, CONTEXT_COUNT_OFFSET, CONTEXT_TABLE_OFFSET, GENERATION_DIGEST_LEN_OFFSET,
    GENERATION_DIGEST_OFFSET, HEADER_LEN, Header, MAGIC, OWNER_COUNT_OFFSET, OWNER_ENTRY_LEN,
    OWNER_TABLE_OFFSET, RELOCATION_COUNT_OFFSET, RELOCATION_ENTRY_LEN, RELOCATION_TABLE_OFFSET,
    ROOT_DIGEST_LEN_OFFSET, ROOT_DIGEST_OFFSET, SELECTOR_COUNT_OFFSET, SELECTOR_ENTRY_LEN,
    SELECTOR_TABLE_OFFSET, STRING_TABLE_OFFSET, TOTAL_LEN_OFFSET, WORKSPACE_ID_LEN_OFFSET,
    WORKSPACE_ID_OFFSET,
};

pub(super) fn decode_header(mapping: &[u8]) -> Result<Header, String> {
    if mapping.len() < HEADER_LEN || &mapping[0..16] != MAGIC {
        return Err("workspace exact projection segment header is invalid".to_owned());
    }
    let total_len = read_usize(mapping, TOTAL_LEN_OFFSET, "segment total length")?;
    if total_len != mapping.len() {
        return Err("workspace exact projection segment length mismatch".to_owned());
    }
    let header = Header {
        epoch: read_u64(mapping, super::EPOCH_OFFSET, "generation epoch")?,
        owner_count: read_usize(mapping, OWNER_COUNT_OFFSET, "owner count")?,
        selector_count: read_usize(mapping, SELECTOR_COUNT_OFFSET, "selector count")?,
        owner_table_offset: read_usize(mapping, OWNER_TABLE_OFFSET, "owner table offset")?,
        selector_table_offset: read_usize(mapping, SELECTOR_TABLE_OFFSET, "selector table offset")?,
        relocation_table_offset: read_usize(
            mapping,
            RELOCATION_TABLE_OFFSET,
            "relocation table offset",
        )?,
        relocation_count: read_usize(mapping, RELOCATION_COUNT_OFFSET, "relocation count")?,
        context_table_offset: read_usize(
            mapping,
            CONTEXT_TABLE_OFFSET,
            "evidence context table offset",
        )?,
        context_count: read_usize(mapping, CONTEXT_COUNT_OFFSET, "evidence context count")?,
        string_table_offset: read_usize(mapping, STRING_TABLE_OFFSET, "string table offset")?,
        blob_offset: read_usize(mapping, BLOB_OFFSET, "blob offset")?,
        generation_digest_offset: read_usize(
            mapping,
            GENERATION_DIGEST_OFFSET,
            "generation digest offset",
        )?,
        generation_digest_len: read_usize(
            mapping,
            GENERATION_DIGEST_LEN_OFFSET,
            "generation digest length",
        )?,
        root_digest_offset: read_usize(mapping, ROOT_DIGEST_OFFSET, "root digest offset")?,
        root_digest_len: read_usize(mapping, ROOT_DIGEST_LEN_OFFSET, "root digest length")?,
        workspace_id_offset: read_usize(mapping, WORKSPACE_ID_OFFSET, "workspace id offset")?,
        workspace_id_len: read_usize(mapping, WORKSPACE_ID_LEN_OFFSET, "workspace id length")?,
    };
    let owner_table_end = header
        .owner_table_offset
        .checked_add(header.owner_count.saturating_mul(OWNER_ENTRY_LEN))
        .ok_or_else(|| "workspace exact owner table overflow".to_owned())?;
    let selector_table_end = header
        .selector_table_offset
        .checked_add(header.selector_count.saturating_mul(SELECTOR_ENTRY_LEN))
        .ok_or_else(|| "workspace exact selector table overflow".to_owned())?;
    let relocation_table_end = header
        .relocation_table_offset
        .checked_add(header.relocation_count.saturating_mul(RELOCATION_ENTRY_LEN))
        .ok_or_else(|| "workspace exact relocation table overflow".to_owned())?;
    let context_table_end = header
        .context_table_offset
        .checked_add(header.context_count.saturating_mul(CONTEXT_ENTRY_LEN))
        .ok_or_else(|| "workspace exact evidence context table overflow".to_owned())?;
    if header.owner_table_offset != HEADER_LEN
        || header.selector_table_offset != owner_table_end
        || header.relocation_table_offset != selector_table_end
        || header.context_table_offset != relocation_table_end
        || header.string_table_offset != context_table_end
        || header.string_table_offset > header.blob_offset
        || header.blob_offset > mapping.len()
    {
        return Err("workspace exact projection table topology is invalid".to_owned());
    }
    Ok(header)
}

pub(super) fn write_range_header(
    target: &mut [u8],
    offset_field: usize,
    len_field: usize,
    base: usize,
    range: (usize, usize),
) -> Result<(), String> {
    write_usize(target, offset_field, base + range.0)?;
    write_usize(target, len_field, range.1)
}

pub(super) fn write_range_entry(
    target: &mut [u8],
    field: usize,
    base: usize,
    range: (usize, usize),
) -> Result<(), String> {
    write_usize(target, field, base + range.0)?;
    write_usize(target, field + 8, range.1)
}

pub(super) fn write_usize(target: &mut [u8], offset: usize, value: usize) -> Result<(), String> {
    let value = u64::try_from(value)
        .map_err(|_| "workspace exact projection value overflows u64".to_owned())?;
    write_u64(target, offset, value)
}

pub(super) fn write_u64(target: &mut [u8], offset: usize, value: u64) -> Result<(), String> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| "workspace exact projection write offset overflow".to_owned())?;
    let destination = target
        .get_mut(offset..end)
        .ok_or_else(|| "workspace exact projection write is out of range".to_owned())?;
    destination.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

pub(super) fn read_usize(bytes: &[u8], offset: usize, field: &str) -> Result<usize, String> {
    usize::try_from(read_u64(bytes, offset, field)?)
        .map_err(|_| format!("workspace exact projection {field} overflows usize"))
}

fn read_u64(bytes: &[u8], offset: usize, field: &str) -> Result<u64, String> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| format!("workspace exact projection {field} offset overflow"))?;
    let raw = bytes
        .get(offset..end)
        .ok_or_else(|| format!("workspace exact projection {field} is out of range"))?;
    Ok(u64::from_le_bytes(raw.try_into().map_err(|_| {
        format!("workspace exact projection {field} is invalid")
    })?))
}

pub(super) fn checked_entry_offset(
    table_offset: usize,
    index: usize,
    entry_len: usize,
    mapping_len: usize,
) -> Result<usize, String> {
    let start = table_offset
        .checked_add(index.saturating_mul(entry_len))
        .ok_or_else(|| "workspace exact projection entry offset overflow".to_owned())?;
    let end = start
        .checked_add(entry_len)
        .ok_or_else(|| "workspace exact projection entry length overflow".to_owned())?;
    if end > mapping_len {
        return Err("workspace exact projection entry is out of range".to_owned());
    }
    Ok(start)
}

pub(super) fn read_slice<'a>(
    bytes: &'a [u8],
    offset: usize,
    len: usize,
    field: &str,
) -> Result<&'a [u8], String> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| format!("workspace exact projection {field} range overflow"))?;
    bytes
        .get(offset..end)
        .ok_or_else(|| format!("workspace exact projection {field} is out of range"))
}

pub(super) fn read_text<'a>(
    bytes: &'a [u8],
    offset: usize,
    len: usize,
    field: &str,
) -> Result<&'a str, String> {
    std::str::from_utf8(read_slice(bytes, offset, len, field)?)
        .map_err(|error| format!("workspace exact projection {field} is not UTF-8: {error}"))
}
