// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Immutable exact-projection segment encoding owned by the workspace publisher.

use super::evidence_context::{CONTEXT_ENTRY_LEN, context_key_hash};
use super::{
    BLOB_OFFSET, CONTEXT_COUNT_OFFSET, CONTEXT_TABLE_OFFSET, EPOCH_OFFSET,
    GENERATION_DIGEST_LEN_OFFSET, GENERATION_DIGEST_OFFSET, HEADER_LEN, MAGIC, OWNER_COUNT_OFFSET,
    OWNER_ENTRY_LEN, OWNER_TABLE_OFFSET, RELOCATION_COUNT_OFFSET, RELOCATION_ENTRY_LEN,
    RELOCATION_TABLE_OFFSET, ROOT_DIGEST_LEN_OFFSET, ROOT_DIGEST_OFFSET, SELECTOR_COUNT_OFFSET,
    SELECTOR_ENTRY_LEN, SELECTOR_TABLE_OFFSET, STRING_TABLE_OFFSET, TOTAL_LEN_OFFSET,
    WORKSPACE_ID_LEN_OFFSET, WORKSPACE_ID_OFFSET, owner_projection_digest, projection_key_hash,
    push_bytes, relocation_identity, relocation_key_hash, slice_from_range, write_range_entry,
    write_range_header, write_u64, write_usize,
};
use crate::runtime_server_workspace::{
    ExactProjectionKind, WorkspaceDerivedProjectionSnapshot, WorkspaceMemoryGeneration,
    WorkspaceOwnerSnapshot, WorkspaceSelectorSnapshot,
};

type ByteRange = (usize, usize);
type OwnerRow = (
    [u8; 32],
    ByteRange,
    ByteRange,
    usize,
    usize,
    ByteRange,
    ByteRange,
    ByteRange,
    [u8; 32],
);
type SelectorRow = (
    [u8; 32],
    ByteRange,
    ByteRange,
    usize,
    usize,
    usize,
    usize,
    usize,
    ByteRange,
);
type EvidenceContexts = std::collections::BTreeMap<String, Vec<u8>>;

struct ProjectionRowSink<'a> {
    strings: &'a mut Vec<u8>,
    blobs: &'a mut Vec<u8>,
    selector_rows: Vec<SelectorRow>,
    evidence_contexts: EvidenceContexts,
}

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
    let mut blobs = Vec::new();
    let (owner_rows, mut selector_rows, evidence_contexts) =
        collect_projection_rows(&owners, selector_count, &mut strings, &mut blobs)?;
    selector_rows.sort_by(|left, right| {
        left.0.cmp(&right.0).then_with(|| {
            slice_from_range(&strings, left.2)
                .cmp(slice_from_range(&strings, right.2))
                .then_with(|| {
                    slice_from_range(&strings, left.1).cmp(slice_from_range(&strings, right.1))
                })
        })
    });
    let mut relocation_rows = Vec::new();
    for (selector_index, row) in selector_rows.iter().enumerate() {
        let projection_kind = std::str::from_utf8(slice_from_range(&strings, row.2))
            .map_err(|error| format!("workspace exact projection kind is not UTF-8: {error}"))?;
        if projection_kind != "source" {
            continue;
        }
        let selector = std::str::from_utf8(slice_from_range(&strings, row.1))
            .map_err(|error| format!("workspace exact selector is not UTF-8: {error}"))?;
        let identity = relocation_identity(selector)?;
        relocation_rows.push((relocation_key_hash(identity.as_str()), selector_index));
    }
    relocation_rows.sort_unstable();
    let context_table_offset = relocation_table_offset
        .checked_add(relocation_rows.len().saturating_mul(RELOCATION_ENTRY_LEN))
        .ok_or_else(|| "workspace exact relocation table length overflow".to_owned())?;
    let string_table_offset = context_table_offset
        .checked_add(evidence_contexts.len().saturating_mul(CONTEXT_ENTRY_LEN))
        .ok_or_else(|| "workspace exact evidence context table length overflow".to_owned())?;
    let mut context_rows = evidence_contexts
        .into_iter()
        .map(|(evidence_context_ref, context_bytes)| {
            let reference = push_bytes(&mut strings, evidence_context_ref.as_bytes());
            let context_blob_offset = blobs.len();
            blobs.extend_from_slice(&context_bytes);
            (
                context_key_hash(&evidence_context_ref),
                reference,
                context_blob_offset,
                context_bytes.len(),
            )
        })
        .collect::<Vec<_>>();
    context_rows.sort_by(|left, right| {
        left.0.cmp(&right.0).then_with(|| {
            slice_from_range(&strings, left.1).cmp(slice_from_range(&strings, right.1))
        })
    });
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
    write_usize(&mut segment, CONTEXT_TABLE_OFFSET, context_table_offset)?;
    write_usize(&mut segment, CONTEXT_COUNT_OFFSET, context_rows.len())?;
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
    for (
        index,
        (
            hash,
            path,
            digest,
            owner_blob_offset,
            owner_blob_len,
            language,
            provider,
            diagnostic,
            projection_digest,
        ),
    ) in owner_rows.into_iter().enumerate()
    {
        let start = owner_table_offset + index * OWNER_ENTRY_LEN;
        segment[start..start + 32].copy_from_slice(&hash);
        write_range_entry(&mut segment, start + 32, string_table_offset, path)?;
        write_range_entry(&mut segment, start + 48, string_table_offset, digest)?;
        write_usize(&mut segment, start + 64, blob_offset + owner_blob_offset)?;
        write_usize(&mut segment, start + 72, owner_blob_len)?;
        write_range_entry(&mut segment, start + 80, string_table_offset, language)?;
        write_range_entry(&mut segment, start + 96, string_table_offset, provider)?;
        write_range_entry(&mut segment, start + 112, string_table_offset, diagnostic)?;
        segment[start + 128..start + 160].copy_from_slice(&projection_digest);
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
            query_keys,
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
        write_range_entry(&mut segment, start + 104, string_table_offset, query_keys)?;
    }
    for (index, (hash, selector_index)) in relocation_rows.into_iter().enumerate() {
        let start = relocation_table_offset + index * RELOCATION_ENTRY_LEN;
        segment[start..start + 32].copy_from_slice(&hash);
        write_usize(&mut segment, start + 32, selector_index)?;
    }
    for (index, (hash, reference, context_blob_offset, context_blob_len)) in
        context_rows.into_iter().enumerate()
    {
        let start = context_table_offset + index * CONTEXT_ENTRY_LEN;
        segment[start..start + 32].copy_from_slice(&hash);
        write_range_entry(&mut segment, start + 32, string_table_offset, reference)?;
        write_usize(&mut segment, start + 48, blob_offset + context_blob_offset)?;
        write_usize(&mut segment, start + 56, context_blob_len)?;
    }
    segment[string_table_offset..blob_offset].copy_from_slice(&strings);
    segment[blob_offset..].copy_from_slice(&blobs);
    Ok(segment)
}

fn collect_projection_rows(
    owners: &[&WorkspaceOwnerSnapshot],
    selector_count: usize,
    strings: &mut Vec<u8>,
    blobs: &mut Vec<u8>,
) -> Result<(Vec<OwnerRow>, Vec<SelectorRow>, EvidenceContexts), String> {
    let mut owner_rows = Vec::with_capacity(owners.len());
    let mut sink = ProjectionRowSink {
        strings,
        blobs,
        selector_rows: Vec::with_capacity(selector_count),
        evidence_contexts: EvidenceContexts::new(),
    };
    for (owner_index, owner) in owners.iter().enumerate() {
        append_owner_projection_rows(owner_index, owner, &mut owner_rows, &mut sink)?;
    }
    Ok((owner_rows, sink.selector_rows, sink.evidence_contexts))
}

fn append_owner_projection_rows(
    owner_index: usize,
    owner: &WorkspaceOwnerSnapshot,
    owner_rows: &mut Vec<OwnerRow>,
    sink: &mut ProjectionRowSink<'_>,
) -> Result<(), String> {
    let path = push_bytes(sink.strings, owner.owner_path.as_bytes());
    let digest = push_bytes(sink.strings, owner.content_digest.as_bytes());
    let (language, provider) = if let Some(authority) = &owner.authority {
        (
            push_bytes(sink.strings, authority.language_id.as_str().as_bytes()),
            push_bytes(sink.strings, authority.provider_id.as_str().as_bytes()),
        )
    } else {
        ((sink.strings.len(), 0), (sink.strings.len(), 0))
    };
    let diagnostic = if let Some(diagnostic) = &owner.native_syntax_diagnostic {
        let bytes = serde_json::to_vec(diagnostic)
            .map_err(|error| format!("encode owner native syntax diagnostic: {error}"))?;
        push_bytes(sink.strings, &bytes)
    } else {
        (sink.strings.len(), 0)
    };
    let blob_offset = sink.blobs.len();
    sink.blobs.extend_from_slice(&owner.bytes);
    owner_rows.push((
        *blake3::hash(owner.owner_path.as_bytes()).as_bytes(),
        path,
        digest,
        blob_offset,
        owner.bytes.len(),
        language,
        provider,
        diagnostic,
        owner_projection_digest(owner),
    ));
    for selector in &owner.selectors {
        append_selector_projection_rows(owner_index, selector, sink)?;
    }
    Ok(())
}

fn append_selector_projection_rows(
    owner_index: usize,
    selector: &WorkspaceSelectorSnapshot,
    sink: &mut ProjectionRowSink<'_>,
) -> Result<(), String> {
    let text = push_bytes(sink.strings, selector.selector.as_bytes());
    let query_keys = serde_json::to_vec(&selector.query_keys)
        .map_err(|error| format!("encode selector query keys: {error}"))?;
    let query_keys = push_bytes(sink.strings, &query_keys);
    let source_kind = push_bytes(sink.strings, b"source");
    sink.selector_rows.push((
        projection_key_hash("source", &selector.selector),
        text,
        source_kind,
        owner_index,
        selector.byte_start,
        selector.byte_end,
        0,
        0,
        query_keys,
    ));
    for projection in &selector.derived_projections {
        append_derived_projection_row(owner_index, selector, projection, text, query_keys, sink)?;
    }
    Ok(())
}

fn append_derived_projection_row(
    owner_index: usize,
    selector: &WorkspaceSelectorSnapshot,
    projection: &WorkspaceDerivedProjectionSnapshot,
    text: ByteRange,
    query_keys: ByteRange,
    sink: &mut ProjectionRowSink<'_>,
) -> Result<(), String> {
    let projection_kind = push_bytes(sink.strings, projection.projection_kind.as_bytes());
    let (projection_bytes, evidence_context) = resident_projection_payload(projection)?;
    if let Some(context) = evidence_context {
        let context_bytes = serde_json::to_vec(&context)
            .map_err(|error| format!("encode projection evidence context: {error}"))?;
        if let Some(existing) = sink.evidence_contexts.insert(
            context.evidence_context_ref.as_str().to_owned(),
            context_bytes.clone(),
        ) && existing != context_bytes
        {
            return Err(
                "projection evidence context ref resolved to conflicting identity".to_owned(),
            );
        }
    }
    let projection_blob_offset = sink.blobs.len();
    sink.blobs.extend_from_slice(&projection_bytes);
    sink.selector_rows.push((
        projection_key_hash(projection.projection_kind.as_str(), &selector.selector),
        text,
        projection_kind,
        owner_index,
        selector.byte_start,
        selector.byte_end,
        projection_blob_offset,
        projection_bytes.len(),
        query_keys,
    ));
    Ok(())
}

fn resident_projection_payload(
    projection: &WorkspaceDerivedProjectionSnapshot,
) -> Result<
    (
        Vec<u8>,
        Option<
            agent_semantic_content_identity::projection_evidence_context::ProjectionEvidenceContext,
        >,
    ),
    String,
> {
    if projection.projection_kind != ExactProjectionKind::CallableSkeleton {
        return Ok((
            projection.bytes.clone(),
            projection.evidence_context.clone(),
        ));
    }
    let envelope: agent_semantic_content_identity::semantic_projection::SemanticProjection<
        agent_semantic_content_identity::callable_skeleton_projection::CallableSkeletonPayload,
    > = serde_json::from_slice(&projection.bytes)
        .map_err(|error| format!("decode callable-skeleton projection for publication: {error}"))?;
    envelope
        .validate()
        .map_err(|error| format!("validate semantic projection envelope: {error}"))?;
    envelope
        .payload
        .validate()
        .map_err(|error| format!("validate callable-skeleton payload: {error}"))?;
    envelope
        .payload
        .validate_scope(envelope.root_selector.as_str())
        .map_err(|error| format!("validate callable-skeleton scope: {error}"))?;
    let context = projection
        .evidence_context
        .clone()
        .ok_or_else(|| "callable-skeleton projection is missing its evidence context".to_owned())?;
    context
        .validate()
        .map_err(|error| format!("validate projection evidence context: {error}"))?;
    if context.evidence_context_ref != envelope.evidence_context_ref
        || context.language_id != envelope.language_id
        || context.provider_id != envelope.provider_id
    {
        return Err("callable-skeleton projection evidence context drift".to_owned());
    }
    Ok((projection.bytes.clone(), Some(context)))
}
