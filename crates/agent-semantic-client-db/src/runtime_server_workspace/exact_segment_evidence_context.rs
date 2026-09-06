// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Evidence-context lookup and snapshot recovery for the immutable exact mmap.

use std::cmp::Ordering;

use super::format::{checked_entry_offset, read_slice, read_text, read_usize};
use super::{MappedWorkspaceExactProjection, SelectorEntry};

pub(super) const CONTEXT_ENTRY_LEN: usize = 64;

impl MappedWorkspaceExactProjection {
    pub(super) fn projection_evidence_context(
        &self,
        evidence_context_ref: &str,
    ) -> Result<Option<Vec<u8>>, String> {
        let hash = context_key_hash(evidence_context_ref);
        let mut low = 0;
        let mut high = self.context_count;
        while low < high {
            let middle = low + (high - low) / 2;
            let entry = self.context_entry(middle)?;
            match entry.hash.cmp(&hash) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => {
                    return self.context_collision_lookup(middle, hash, evidence_context_ref);
                }
            }
        }
        Ok(None)
    }

    fn context_collision_lookup(
        &self,
        middle: usize,
        hash: [u8; 32],
        evidence_context_ref: &str,
    ) -> Result<Option<Vec<u8>>, String> {
        let mut first = middle;
        while first > 0 && self.context_entry(first - 1)?.hash == hash {
            first -= 1;
        }
        for index in first..self.context_count {
            let candidate = self.context_entry(index)?;
            if candidate.hash != hash {
                break;
            }
            if self.context_ref(&candidate)? == evidence_context_ref {
                return read_slice(
                    &self.mapping,
                    candidate.blob_offset,
                    candidate.blob_len,
                    "projection evidence context",
                )
                .map(|bytes| Some(bytes.to_vec()));
            }
        }
        Ok(None)
    }

    fn context_entry(&self, index: usize) -> Result<ContextEntry, String> {
        if index >= self.context_count {
            return Err("workspace exact evidence context index is out of range".to_owned());
        }
        let start = checked_entry_offset(
            self.context_table_offset,
            index,
            CONTEXT_ENTRY_LEN,
            self.mapping.len(),
        )?;
        let bytes = &self.mapping[start..start + CONTEXT_ENTRY_LEN];
        Ok(ContextEntry {
            hash: bytes[0..32]
                .try_into()
                .map_err(|_| "workspace exact evidence context hash is invalid".to_owned())?,
            ref_offset: read_usize(bytes, 32, "evidence context ref offset")?,
            ref_len: read_usize(bytes, 40, "evidence context ref length")?,
            blob_offset: read_usize(bytes, 48, "evidence context blob offset")?,
            blob_len: read_usize(bytes, 56, "evidence context blob length")?,
        })
    }

    fn context_ref<'a>(&'a self, context: &ContextEntry) -> Result<&'a str, String> {
        read_text(
            &self.mapping,
            context.ref_offset,
            context.ref_len,
            "projection evidence context ref",
        )
    }

    pub(super) fn evidence_context_for_projection(
        &self,
        selector: &SelectorEntry,
    ) -> Result<
        Option<
            agent_semantic_content_identity::projection_evidence_context::ProjectionEvidenceContext,
        >,
        String,
    > {
        let bytes = read_slice(
            &self.mapping,
            selector.projection_blob_offset,
            selector.projection_blob_len,
            "derived projection bytes",
        )?;
        let envelope: agent_semantic_content_identity::semantic_projection::SemanticProjection<
            agent_semantic_content_identity::callable_skeleton_projection::CallableSkeletonPayload,
        > = serde_json::from_slice(bytes)
            .map_err(|error| format!("decode resident callable-skeleton projection: {error}"))?;
        envelope
            .validate()
            .map_err(|error| format!("validate resident semantic projection: {error}"))?;
        envelope
            .payload
            .validate()
            .map_err(|error| format!("validate resident callable-skeleton payload: {error}"))?;
        envelope
            .payload
            .validate_scope(envelope.root_selector.as_str())
            .map_err(|error| format!("validate resident callable-skeleton scope: {error}"))?;
        let context = self
            .projection_evidence_context(envelope.evidence_context_ref.as_str())?
            .ok_or_else(|| "resident callable-skeleton evidence context is missing".to_owned())?;
        let context: agent_semantic_content_identity::projection_evidence_context::ProjectionEvidenceContext = serde_json::from_slice(&context)
            .map_err(|error| format!("decode resident projection evidence context: {error}"))?;
        context
            .validate()
            .map_err(|error| format!("validate resident projection evidence context: {error}"))?;
        if context.evidence_context_ref != envelope.evidence_context_ref
            || context.language_id != envelope.language_id
            || context.provider_id != envelope.provider_id
        {
            return Err("resident callable-skeleton evidence context identity drift".to_owned());
        }
        Ok(Some(context))
    }
}

#[derive(Clone, Copy)]
struct ContextEntry {
    hash: [u8; 32],
    ref_offset: usize,
    ref_len: usize,
    blob_offset: usize,
    blob_len: usize,
}

pub(super) fn context_key_hash(evidence_context_ref: &str) -> [u8; 32] {
    *blake3::hash(evidence_context_ref.as_bytes()).as_bytes()
}
