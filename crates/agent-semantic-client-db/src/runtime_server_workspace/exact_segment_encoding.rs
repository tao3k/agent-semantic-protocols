//! Immutable exact-projection segment encoding owned by the workspace publisher.

use super::{
    BLOB_OFFSET, EPOCH_OFFSET, GENERATION_DIGEST_LEN_OFFSET, GENERATION_DIGEST_OFFSET, HEADER_LEN,
    MAGIC, OWNER_COUNT_OFFSET, OWNER_ENTRY_LEN, OWNER_TABLE_OFFSET, RELOCATION_COUNT_OFFSET,
    RELOCATION_ENTRY_LEN, RELOCATION_TABLE_OFFSET, ROOT_DIGEST_LEN_OFFSET, ROOT_DIGEST_OFFSET,
    SELECTOR_COUNT_OFFSET, SELECTOR_ENTRY_LEN, SELECTOR_TABLE_OFFSET, STRING_TABLE_OFFSET,
    TOTAL_LEN_OFFSET, WORKSPACE_ID_LEN_OFFSET, WORKSPACE_ID_OFFSET, owner_projection_digest,
    projection_key_hash, push_bytes, relocation_identity, relocation_key_hash, slice_from_range,
    write_range_entry, write_range_header, write_u64, write_usize,
};
use crate::runtime_server_workspace::WorkspaceMemoryGeneration;

pub(crate) fn encode_exact_projection_segment(
    generation: &WorkspaceMemoryGeneration,
) -> Result<Vec<u8>, String> {
    generation.validate()?;
    let mut owners = generation.owners.iter().collect::<Vec<_>>();
    owners.sort_by(|left, right| {
        blake3::hash(left.owner_path.as_bytes())
            .as_bytes()
            .cmp(blake3::hash(right.owner_path.as_bytes()).as_bytes())
            .then_with(|| left.owner_path.cmp(&right.owner_path))
    });
    let owner_table_offset = HEADER_LEN;
    let selector_count = owners
        .iter()
        .flat_map(|owner| owner.selectors.iter())
        .map(|selector| 1 + selector.derived_projections.len())
        .sum::<usize>();
    let selector_table_offset = owner_table_offset
        .checked_add(owners.len().saturating_mul(OWNER_ENTRY_LEN))
        .ok_or_else(|| "workspace exact owner table length overflow".to_owned())?;
    let relocation_table_offset = selector_table_offset
        .checked_add(selector_count.saturating_mul(SELECTOR_ENTRY_LEN))
        .ok_or_else(|| "workspace exact selector table length overflow".to_owned())?;

    let mut strings = Vec::new();
    let generation_digest = push_bytes(&mut strings, generation.generation_digest.as_bytes());
    let root_digest = push_bytes(
        &mut strings,
        generation.source_snapshot.root_digest.as_bytes(),
    );
    let workspace_identity = push_bytes(&mut strings, generation.workspace_identity.as_bytes());
    let mut owner_rows = Vec::with_capacity(owners.len());
    let mut selector_rows = Vec::with_capacity(selector_count);
    let mut blobs = Vec::new();
    for (owner_index, owner) in owners.iter().enumerate() {
        let path = push_bytes(&mut strings, owner.owner_path.as_bytes());
        let digest = push_bytes(&mut strings, owner.content_digest.as_bytes());
        let blob_offset = blobs.len();
        blobs.extend_from_slice(&owner.bytes);
        owner_rows.push((
            *blake3::hash(owner.owner_path.as_bytes()).as_bytes(),
            path,
            digest,
            blob_offset,
            owner.bytes.len(),
            owner_projection_digest(owner),
        ));
        for selector in &owner.selectors {
            let text = push_bytes(&mut strings, selector.selector.as_bytes());
            let source_kind = push_bytes(&mut strings, b"source");
            selector_rows.push((
                projection_key_hash("source", &selector.selector),
                text,
                source_kind,
                owner_index,
                selector.byte_start,
                selector.byte_end,
                0,
                0,
            ));
            for projection in &selector.derived_projections {
                let projection_kind =
                    push_bytes(&mut strings, projection.projection_kind.as_bytes());
                let projection_blob_offset = blobs.len();
                blobs.extend_from_slice(&projection.bytes);
                selector_rows.push((
                    projection_key_hash(&projection.projection_kind, &selector.selector),
                    text,
                    projection_kind,
                    owner_index,
                    selector.byte_start,
                    selector.byte_end,
                    projection_blob_offset,
                    projection.bytes.len(),
                ));
            }
        }
    }
    selector_rows.sort_by(|left, right| {
        left.0.cmp(&right.0).then_with(|| {
            slice_from_range(&strings, left.2)
                .cmp(slice_from_range(&strings, right.2))
                .then_with(|| {
                    slice_from_range(&strings, left.1).cmp(slice_from_range(&strings, right.1))
                })
        })
    });
    let mut relocation_rows = selector_rows
        .iter()
        .enumerate()
        .map(|(selector_index, row)| {
            let selector = std::str::from_utf8(slice_from_range(&strings, row.1))
                .map_err(|error| format!("workspace exact selector is not UTF-8: {error}"))?;
            let projection_kind =
                std::str::from_utf8(slice_from_range(&strings, row.2)).map_err(|error| {
                    format!("workspace exact projection kind is not UTF-8: {error}")
                })?;
            let identity = relocation_identity(selector)?;
            Ok((
                relocation_key_hash(projection_kind, identity.as_str()),
                selector_index,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    relocation_rows.sort_unstable();
    let string_table_offset = relocation_table_offset
        .checked_add(relocation_rows.len().saturating_mul(RELOCATION_ENTRY_LEN))
        .ok_or_else(|| "workspace exact relocation table length overflow".to_owned())?;
    let blob_offset = string_table_offset
        .checked_add(strings.len())
        .ok_or_else(|| "workspace exact string table length overflow".to_owned())?;
    let total_len = blob_offset
        .checked_add(blobs.len())
        .ok_or_else(|| "workspace exact segment length overflow".to_owned())?;
    let mut segment = vec![0_u8; total_len];
    segment[0..16].copy_from_slice(MAGIC);
    write_u64(&mut segment, EPOCH_OFFSET, generation.active_epoch)?;
    write_usize(&mut segment, OWNER_COUNT_OFFSET, owners.len())?;
    write_usize(&mut segment, SELECTOR_COUNT_OFFSET, selector_rows.len())?;
    write_usize(&mut segment, OWNER_TABLE_OFFSET, owner_table_offset)?;
    write_usize(&mut segment, SELECTOR_TABLE_OFFSET, selector_table_offset)?;
    write_usize(
        &mut segment,
        RELOCATION_TABLE_OFFSET,
        relocation_table_offset,
    )?;
    write_usize(&mut segment, RELOCATION_COUNT_OFFSET, relocation_rows.len())?;
    write_usize(&mut segment, STRING_TABLE_OFFSET, string_table_offset)?;
    write_usize(&mut segment, BLOB_OFFSET, blob_offset)?;
    write_usize(&mut segment, TOTAL_LEN_OFFSET, total_len)?;
    write_range_header(
        &mut segment,
        GENERATION_DIGEST_OFFSET,
        GENERATION_DIGEST_LEN_OFFSET,
        string_table_offset,
        generation_digest,
    )?;
    write_range_header(
        &mut segment,
        ROOT_DIGEST_OFFSET,
        ROOT_DIGEST_LEN_OFFSET,
        string_table_offset,
        root_digest,
    )?;
    write_range_header(
        &mut segment,
        WORKSPACE_ID_OFFSET,
        WORKSPACE_ID_LEN_OFFSET,
        string_table_offset,
        workspace_identity,
    )?;
    for (index, (hash, path, digest, owner_blob_offset, owner_blob_len, projection_digest)) in
        owner_rows.into_iter().enumerate()
    {
        let start = owner_table_offset + index * OWNER_ENTRY_LEN;
        segment[start..start + 32].copy_from_slice(&hash);
        write_range_entry(&mut segment, start + 32, string_table_offset, path)?;
        write_range_entry(&mut segment, start + 48, string_table_offset, digest)?;
        write_usize(&mut segment, start + 64, blob_offset + owner_blob_offset)?;
        write_usize(&mut segment, start + 72, owner_blob_len)?;
        segment[start + 80..start + 112].copy_from_slice(&projection_digest);
    }
    for (
        index,
        (
            hash,
            selector,
            projection_kind,
            owner_index,
            byte_start,
            byte_end,
            projection_blob_offset,
            projection_blob_len,
        ),
    ) in selector_rows.into_iter().enumerate()
    {
        let start = selector_table_offset + index * SELECTOR_ENTRY_LEN;
        segment[start..start + 32].copy_from_slice(&hash);
        write_range_entry(&mut segment, start + 32, string_table_offset, selector)?;
        write_range_entry(
            &mut segment,
            start + 48,
            string_table_offset,
            projection_kind,
        )?;
        write_usize(&mut segment, start + 64, owner_index)?;
        write_usize(&mut segment, start + 72, byte_start)?;
        write_usize(&mut segment, start + 80, byte_end)?;
        write_usize(
            &mut segment,
            start + 88,
            blob_offset + projection_blob_offset,
        )?;
        write_usize(&mut segment, start + 96, projection_blob_len)?;
    }
    for (index, (hash, selector_index)) in relocation_rows.into_iter().enumerate() {
        let start = relocation_table_offset + index * RELOCATION_ENTRY_LEN;
        segment[start..start + 32].copy_from_slice(&hash);
        write_usize(&mut segment, start + 32, selector_index)?;
    }
    segment[string_table_offset..blob_offset].copy_from_slice(&strings);
    segment[blob_offset..].copy_from_slice(&blobs);
    Ok(segment)
}
