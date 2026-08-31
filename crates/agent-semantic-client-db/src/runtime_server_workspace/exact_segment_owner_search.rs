//! Compact owner-search reads over the immutable exact-projection segment.

use super::{MappedWorkspaceExactProjection, SelectorEntry, read_slice};
use crate::runtime_server_workspace::{ExactProjectionKind, WorkspaceOwnerSearchSeedSnapshot};

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
        let (candidate_count, mut seeds) = indices.iter().try_fold(
            (0usize, Vec::with_capacity(indices.len().min(limit))),
            |(candidate_count, mut seeds), index| {
                let entry = self.selector_entry(*index)?;
                if self.selector_kind(&entry)? != ExactProjectionKind::Source
                    || !self.selector_query_keys_match(&entry, query_terms)?
                {
                    return Ok::<_, String>((candidate_count, seeds));
                }
                if seeds.len() < limit {
                    seeds.push(self.owner_search_seed(entry)?);
                }
                Ok((candidate_count + 1, seeds))
            },
        )?;
        seeds.sort_by(|left, right| left.selector.cmp(&right.selector));
        if seeds
            .windows(2)
            .any(|pair| pair[0].selector == pair[1].selector)
        {
            return Err(
                "workspace exact projection contains duplicate source selectors".to_owned(),
            );
        }
        Ok((candidate_count, seeds))
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

    fn selector_query_keys_match(
        &self,
        selector: &SelectorEntry,
        query_terms: &[String],
    ) -> Result<bool, String> {
        if query_terms.is_empty() {
            return Ok(true);
        }
        let keys: Vec<&str> = serde_json::from_slice(read_slice(
            &self.mapping,
            selector.query_keys_offset,
            selector.query_keys_len,
            "selector query keys",
        )?)
        .map_err(|error| format!("decode workspace selector query keys: {error}"))?;
        if keys.iter().any(|key| key.is_empty()) || keys.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("workspace selector query keys are not canonical".to_owned());
        }
        Ok(query_terms
            .iter()
            .any(|term| keys.binary_search(&term.as_str()).is_ok()))
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
