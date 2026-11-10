// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Byte-gram candidate filtering over resident owner deltas.

use std::collections::BTreeSet;

use super::resident_content_overlay::OwnerContentOverlayValue;
use super::resident_overlay::ResidentOverlaySnapshot;

impl ResidentOverlaySnapshot {
    pub(super) fn merge_grep_candidates(
        &self,
        base_candidates: Vec<String>,
        plan: &agent_semantic_search::ResidentGrepCandidatePlan,
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: usize,
    ) -> Result<Vec<String>, String> {
        let mut candidates = base_candidates
            .into_iter()
            .filter(|owner_path| {
                !self.state.tombstones.contains(owner_path)
                    && !self.state.owners.contains_key(owner_path)
                    && self.state.content_owners.get(owner_path).is_none()
            })
            .collect::<BTreeSet<_>>();
        for (owner_path, owner) in self.state.owners.iter() {
            if self.state.tombstones.contains(owner_path)
                || authority.is_some_and(|required| owner.authority.as_ref() != Some(required))
            {
                continue;
            }
            let grams = self
                .state
                .owner_grams
                .get(owner_path)
                .ok_or_else(|| "resident owner overlay gram index is missing".to_owned())?;
            if owner_grams_match_plan(grams, plan) {
                candidates.insert(owner_path.clone());
            }
        }
        for (owner_path, value) in self.state.content_owners.entries() {
            let OwnerContentOverlayValue::Present { owner, grams } = value else {
                continue;
            };
            if authority.is_some_and(|required| owner.authority.as_ref() != Some(required)) {
                continue;
            }
            if owner_grams_match_plan(grams, plan) {
                candidates.insert(owner_path);
            }
        }
        if candidates.len() > limit {
            return Err(format!(
                "query-not-ready: resident GREP candidate budget exceeded: candidates={} limit={limit}",
                candidates.len()
            ));
        }
        Ok(candidates.into_iter().collect())
    }
}

pub(super) fn owner_byte_grams(bytes: &[u8]) -> BTreeSet<u32> {
    bytes
        .windows(agent_semantic_search::RESIDENT_BYTE_GRAM_WIDTH)
        .map(|window| {
            (u32::from(window[0]) << 16) | (u32::from(window[1]) << 8) | u32::from(window[2])
        })
        .collect()
}

fn owner_grams_match_plan(
    grams: &BTreeSet<u32>,
    plan: &agent_semantic_search::ResidentGrepCandidatePlan,
) -> bool {
    match plan {
        agent_semantic_search::ResidentGrepCandidatePlan::MatchAll => true,
        agent_semantic_search::ResidentGrepCandidatePlan::Grams(required) => {
            required.iter().all(|gram| grams.contains(gram))
        }
        agent_semantic_search::ResidentGrepCandidatePlan::And(plans) => {
            plans.iter().all(|plan| owner_grams_match_plan(grams, plan))
        }
        agent_semantic_search::ResidentGrepCandidatePlan::Or(plans) => {
            plans.iter().any(|plan| owner_grams_match_plan(grams, plan))
        }
    }
}
