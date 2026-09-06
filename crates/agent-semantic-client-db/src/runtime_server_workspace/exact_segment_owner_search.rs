// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Compact owner-search reads over the immutable exact-projection segment.

use super::{MappedWorkspaceExactProjection, SelectorEntry, read_slice};
use crate::runtime_server_workspace::WorkspaceOwnerSearchSeedSnapshot;

impl MappedWorkspaceExactProjection {
    pub(super) fn owner_search_seeds(
        &self,
        owner_index: usize,
        query_terms: &[String],
        limit: usize,
    ) -> Result<(usize, Vec<WorkspaceOwnerSearchSeedSnapshot>), String> {
        let indices = self
            .selector_indices_by_owner
            .get(owner_index)
            .ok_or_else(|| "workspace exact projection owner index is out of range".to_owned())?;
        let seeds = indices.iter().try_fold(
            Vec::with_capacity(indices.len()),
            |mut seeds, index| {
                let entry = self.selector_entry(*index)?;
                let projection_kind = self.selector_kind(&entry)?;
                let query_keys = self.selector_query_keys(&entry)?;
                if !crate::runtime_server_workspace::owner_search_admission::selector_is_admitted(
                    projection_kind,
                    &query_keys,
                    query_terms,
                )? {
                    return Ok::<_, String>(seeds);
                }
                seeds.push(self.owner_search_seed(entry)?);
                Ok(seeds)
            },
        )?;
        crate::runtime_server_workspace::owner_search_admission::finish_owner_search(seeds, limit)
    }

    fn owner_search_seed(
        &self,
        entry: SelectorEntry,
    ) -> Result<WorkspaceOwnerSearchSeedSnapshot, String> {
        Ok(WorkspaceOwnerSearchSeedSnapshot {
            selector: self.selector_text(&entry)?.to_owned(),
            byte_start: entry.byte_start,
            byte_end: entry.byte_end,
        })
    }

    pub(super) fn selector_query_keys(
        &self,
        selector: &SelectorEntry,
    ) -> Result<Vec<String>, String> {
        let keys: Vec<String> = serde_json::from_slice(read_slice(
            &self.mapping,
            selector.query_keys_offset,
            selector.query_keys_len,
            "selector query keys",
        )?)
        .map_err(|error| format!("decode workspace selector query keys: {error}"))?;
        if keys.iter().any(|key| key.is_empty()) || keys.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("workspace selector query keys are not canonical".to_owned());
        }
        Ok(keys)
    }
}
