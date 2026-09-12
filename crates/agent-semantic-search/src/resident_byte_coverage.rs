// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Immutable generation-bound trigram candidate coverage for the V1 rg axis.
//!
//! GREP owns fast lexical anchoring; Tantivy contributes ranked lexical recall,
//! and native syntax refines their fused owner scope into structural evidence.
//! This attachment narrows an admitted rg pattern to a candidate owner
//! superset. The exact resident matcher remains authoritative. Its build
//! representation is discarded after publication; mapped generations retain
//! no workspace-sized trigram `HashMap` on the request heap.

use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::Arc;

use crate::{ResidentGrepCandidatePlan, ResidentSearchAuthority};

#[path = "resident_byte_coverage_format.rs"]
mod format;

pub const RESIDENT_BYTE_GRAM_WIDTH: usize = 3;

pub struct ResidentByteCoverageInput<'a> {
    pub owner_path: String,
    pub authority: Option<ResidentSearchAuthority>,
    pub bytes: &'a [u8],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentByteCoverageOwner {
    pub owner_path: String,
    pub authority: Option<ResidentSearchAuthority>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidentByteCoverageStats {
    pub owner_count: usize,
    pub gram_count: usize,
    pub posting_count: usize,
    pub artifact_bytes: usize,
    pub artifact_heap_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentByteCoverageQueryReceipt {
    pub requested_gram_count: usize,
    pub decoded_posting_count: usize,
    pub smallest_posting_count: usize,
    pub candidate_count: usize,
    pub lookup_nanos: u64,
}

enum ResidentByteCoverageStorage {
    Owned(Arc<[u8]>),
    Mapped {
        mapping: Arc<memmap2::Mmap>,
        range: Range<usize>,
    },
}

impl ResidentByteCoverageStorage {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Mapped { mapping, range } => &mapping[range.clone()],
        }
    }

    fn artifact_heap_bytes(&self) -> usize {
        match self {
            Self::Owned(bytes) => bytes.len(),
            Self::Mapped { .. } => 0,
        }
    }
}

pub struct ResidentByteCoverageIndex {
    owners: Vec<ResidentByteCoverageOwner>,
    storage: ResidentByteCoverageStorage,
    layout: format::ValidatedByteCoverageLayout,
}

impl std::fmt::Debug for ResidentByteCoverageIndex {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResidentByteCoverageIndex")
            .field("stats", &self.stats())
            .finish()
    }
}

impl ResidentByteCoverageIndex {
    pub fn new<'a>(
        inputs: impl IntoIterator<Item = ResidentByteCoverageInput<'a>>,
    ) -> Result<Self, String> {
        let (owners, encoded) = Self::encode_validated_artifact(inputs)?;
        Ok(Self {
            owners,
            storage: ResidentByteCoverageStorage::Owned(Arc::from(encoded.bytes)),
            layout: encoded.layout,
        })
    }

    pub fn encode_artifact<'a>(
        inputs: impl IntoIterator<Item = ResidentByteCoverageInput<'a>>,
    ) -> Result<(Vec<ResidentByteCoverageOwner>, Vec<u8>), String> {
        let (owners, encoded) = Self::encode_validated_artifact(inputs)?;
        Ok((owners, encoded.bytes))
    }

    fn encode_validated_artifact<'a>(
        inputs: impl IntoIterator<Item = ResidentByteCoverageInput<'a>>,
    ) -> Result<
        (
            Vec<ResidentByteCoverageOwner>,
            format::EncodedByteCoverageArtifact,
        ),
        String,
    > {
        let mut inputs = inputs.into_iter().collect::<Vec<_>>();
        inputs.sort_unstable_by(|left, right| left.owner_path.cmp(&right.owner_path));
        if inputs
            .windows(2)
            .any(|pair| pair[0].owner_path == pair[1].owner_path)
        {
            return Err("resident byte predicates contain a duplicate owner".to_owned());
        }
        let owners = inputs
            .iter()
            .map(|input| ResidentByteCoverageOwner {
                owner_path: input.owner_path.clone(),
                authority: input.authority.clone(),
            })
            .collect();
        let encoded = format::encode_validated(inputs.iter().map(|input| input.bytes))?;
        Ok((owners, encoded))
    }

    pub fn from_owned_artifact(
        owners: Vec<ResidentByteCoverageOwner>,
        artifact: Vec<u8>,
    ) -> Result<Self, String> {
        let storage = ResidentByteCoverageStorage::Owned(Arc::from(artifact));
        let layout = format::ValidatedByteCoverageLayout::parse(storage.bytes(), owners.len())?;
        Ok(Self {
            owners,
            storage,
            layout,
        })
    }

    pub fn from_mapped_artifact(
        owners: Vec<ResidentByteCoverageOwner>,
        mapping: Arc<memmap2::Mmap>,
        range: Range<usize>,
    ) -> Result<Self, String> {
        if range.start > range.end || range.end > mapping.len() {
            return Err("resident byte predicate mapping range is invalid".to_owned());
        }
        let storage = ResidentByteCoverageStorage::Mapped { mapping, range };
        let layout = format::ValidatedByteCoverageLayout::parse(storage.bytes(), owners.len())?;
        Ok(Self {
            owners,
            storage,
            layout,
        })
    }

    pub fn candidate_owner_ids(
        &self,
        literal: &[u8],
        authority: Option<&ResidentSearchAuthority>,
        candidate_limit: usize,
    ) -> Result<Vec<u32>, String> {
        self.candidate_owner_ids_with_receipt(literal, authority, candidate_limit)
            .map(|(owners, _)| owners)
    }

    pub fn candidate_owner_ids_for_grep_plan(
        &self,
        plan: &ResidentGrepCandidatePlan,
        authority: Option<&ResidentSearchAuthority>,
        candidate_limit: usize,
    ) -> Result<Vec<u32>, String> {
        self.candidate_owner_ids_for_grep_plan_with_receipt(plan, authority, candidate_limit)
            .map(|(owners, _)| owners)
    }

    pub fn candidate_owner_ids_for_grep_plan_with_receipt(
        &self,
        plan: &ResidentGrepCandidatePlan,
        authority: Option<&ResidentSearchAuthority>,
        candidate_limit: usize,
    ) -> Result<(Vec<u32>, ResidentByteCoverageQueryReceipt), String> {
        if plan.is_match_all() {
            return Err(
                "reasonKind=resident-rg-pattern-not-materialized resident GREP regex has no sound trigram candidate plan"
                    .to_owned(),
            );
        }
        if candidate_limit == 0 {
            return Err("query-not-ready: resident GREP candidate budget is zero".to_owned());
        }
        let started = std::time::Instant::now();
        let evaluation = self.evaluate_grep_plan(plan)?;
        let mut candidates = evaluation.owners;
        candidates.retain(|owner_id| {
            usize::try_from(*owner_id)
                .ok()
                .and_then(|owner_id| self.owners.get(owner_id))
                .is_some_and(|owner| {
                    authority.is_none_or(|required| owner.authority.as_ref() == Some(required))
                })
        });
        if candidates.len() > candidate_limit {
            return Err(format!(
                "query-not-ready: resident GREP candidate budget exceeded: candidates={} limit={candidate_limit}",
                candidates.len()
            ));
        }
        let receipt = ResidentByteCoverageQueryReceipt {
            requested_gram_count: plan.distinct_gram_count(),
            decoded_posting_count: evaluation.decoded_posting_count,
            smallest_posting_count: evaluation.smallest_posting_count.unwrap_or(0),
            candidate_count: candidates.len(),
            lookup_nanos: elapsed_nanos(started),
        };
        Ok((candidates, receipt))
    }

    pub fn candidate_owner_paths_for_grep_plan(
        &self,
        plan: &ResidentGrepCandidatePlan,
        authority: Option<&ResidentSearchAuthority>,
        candidate_limit: usize,
    ) -> Result<Vec<String>, String> {
        self.candidate_owner_ids_for_grep_plan(plan, authority, candidate_limit)?
            .into_iter()
            .map(|owner_id| {
                self.owner(owner_id)
                    .map(|owner| owner.owner_path.clone())
                    .ok_or_else(|| "resident GREP candidate owner id is out of range".to_owned())
            })
            .collect()
    }

    pub fn candidate_owner_paths_for_grep_plan_with_receipt(
        &self,
        plan: &ResidentGrepCandidatePlan,
        authority: Option<&ResidentSearchAuthority>,
        candidate_limit: usize,
    ) -> Result<(Vec<String>, ResidentByteCoverageQueryReceipt), String> {
        let (owner_ids, receipt) =
            self.candidate_owner_ids_for_grep_plan_with_receipt(plan, authority, candidate_limit)?;
        let owner_paths = owner_ids
            .into_iter()
            .map(|owner_id| {
                self.owner(owner_id)
                    .map(|owner| owner.owner_path.clone())
                    .ok_or_else(|| "resident GREP candidate owner id is out of range".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((owner_paths, receipt))
    }

    pub fn candidate_owner_ids_with_receipt(
        &self,
        literal: &[u8],
        authority: Option<&ResidentSearchAuthority>,
        candidate_limit: usize,
    ) -> Result<(Vec<u32>, ResidentByteCoverageQueryReceipt), String> {
        if literal.len() < RESIDENT_BYTE_GRAM_WIDTH {
            return Err(format!(
                "query-not-ready: exact byte predicate requires at least {RESIDENT_BYTE_GRAM_WIDTH} bytes"
            ));
        }
        if candidate_limit == 0 {
            return Err(
                "query-not-ready: exact byte predicate candidate budget is zero".to_owned(),
            );
        }
        let started = std::time::Instant::now();
        let grams = literal
            .windows(RESIDENT_BYTE_GRAM_WIDTH)
            .map(|window| format::pack_gram(window[0], window[1], window[2]))
            .collect::<BTreeSet<_>>();
        let evaluation = self.evaluate_grep_plan(&ResidentGrepCandidatePlan::Grams(
            grams.iter().copied().collect(),
        ))?;
        let decoded_posting_count = evaluation.decoded_posting_count;
        let smallest_posting_count = evaluation.smallest_posting_count.unwrap_or(0);
        let mut candidates = evaluation.owners;
        candidates.retain(|owner_id| {
            usize::try_from(*owner_id)
                .ok()
                .and_then(|owner_id| self.owners.get(owner_id))
                .is_some_and(|owner| {
                    authority.is_none_or(|required| owner.authority.as_ref() == Some(required))
                })
        });
        if candidates.len() > candidate_limit {
            return Err(format!(
                "query-not-ready: exact byte predicate candidate budget exceeded: candidates={} limit={candidate_limit}",
                candidates.len()
            ));
        }
        let receipt = ResidentByteCoverageQueryReceipt {
            requested_gram_count: grams.len(),
            decoded_posting_count,
            smallest_posting_count,
            candidate_count: candidates.len(),
            lookup_nanos: elapsed_nanos(started),
        };
        Ok((candidates, receipt))
    }

    pub fn candidate_owner_paths(
        &self,
        literal: &[u8],
        authority: Option<&ResidentSearchAuthority>,
        candidate_limit: usize,
    ) -> Result<Vec<String>, String> {
        self.candidate_owner_ids(literal, authority, candidate_limit)?
            .into_iter()
            .map(|owner_id| {
                self.owner(owner_id)
                    .map(|owner| owner.owner_path.clone())
                    .ok_or_else(|| "resident byte predicate owner id is out of range".to_owned())
            })
            .collect()
    }

    #[must_use]
    pub fn owner(&self, owner_id: u32) -> Option<&ResidentByteCoverageOwner> {
        usize::try_from(owner_id)
            .ok()
            .and_then(|owner_id| self.owners.get(owner_id))
    }

    #[must_use]
    pub fn stats(&self) -> ResidentByteCoverageStats {
        ResidentByteCoverageStats {
            owner_count: self.owners.len(),
            gram_count: self.layout.gram_count(),
            posting_count: self.layout.posting_count(),
            artifact_bytes: self.storage.bytes().len(),
            artifact_heap_bytes: self.storage.artifact_heap_bytes(),
        }
    }

    fn evaluate_grep_plan(
        &self,
        plan: &ResidentGrepCandidatePlan,
    ) -> Result<PlanEvaluation, String> {
        match plan {
            ResidentGrepCandidatePlan::MatchAll => Err(
                "reasonKind=resident-rg-pattern-not-materialized resident GREP regex has no sound trigram candidate plan"
                    .to_owned(),
            ),
            ResidentGrepCandidatePlan::Grams(grams) => {
                let mut grams_by_rarity = Vec::with_capacity(grams.len());
                for gram in grams {
                    let Some(posting_count) =
                        self.layout
                            .lookup_posting_count(self.storage.bytes(), *gram)?
                    else {
                        return Ok(PlanEvaluation {
                            owners: Vec::new(),
                            decoded_posting_count: 0,
                            smallest_posting_count: Some(0),
                        });
                    };
                    grams_by_rarity.push((*gram, posting_count));
                }
                grams_by_rarity.sort_unstable_by_key(|(_, posting_count)| *posting_count);
                let mut decoded_posting_count = 0usize;
                let smallest_posting_count = grams_by_rarity
                    .first()
                    .map(|(_, posting_count)| *posting_count);
                let mut candidates: Option<Vec<u32>> = None;
                for (gram, _) in grams_by_rarity {
                    let decoded = if let Some(existing) = &mut candidates {
                        self.layout.intersect_posting(self.storage.bytes(), gram, existing)?
                    } else {
                        let posting = self.layout.posting(self.storage.bytes(), gram)?
                            .ok_or_else(|| "resident GREP posting disappeared after metadata lookup".to_owned())?;
                        let count = posting.len();
                        candidates = Some(posting);
                        count
                    };
                    decoded_posting_count = decoded_posting_count
                        .checked_add(decoded)
                        .ok_or_else(|| "resident GREP decoded posting count overflows".to_owned())?;
                    // The exact matcher is authoritative. Once the rarest-first
                    // intersection reaches a singleton, decoding another
                    // common delta stream can only retain or remove that owner;
                    // exact verification must read it either way.
                    if candidates
                        .as_ref()
                        .is_some_and(|candidates| candidates.len() <= 1)
                    {
                        break;
                    }
                }
                Ok(PlanEvaluation {
                    owners: candidates.ok_or_else(|| {
                        "resident GREP candidate plan contains no postings".to_owned()
                    })?,
                    decoded_posting_count,
                    smallest_posting_count,
                })
            }
            ResidentGrepCandidatePlan::And(plans) => {
                let evaluations = plans
                    .iter()
                    .map(|plan| self.evaluate_grep_plan(plan))
                    .collect::<Result<Vec<_>, _>>()?;
                let decoded_posting_count = sum_decoded_postings(&evaluations)?;
                let smallest_posting_count = smallest_posting(&evaluations);
                let mut candidate_sets = evaluations
                    .into_iter()
                    .map(|evaluation| evaluation.owners)
                    .collect::<Vec<_>>();
                candidate_sets.sort_unstable_by_key(Vec::len);
                Ok(PlanEvaluation {
                    owners: intersect_postings(candidate_sets)?,
                    decoded_posting_count,
                    smallest_posting_count,
                })
            }
            ResidentGrepCandidatePlan::Or(plans) => {
                let mut union = Vec::new();
                let mut decoded_posting_count = 0usize;
                let mut smallest_posting_count = None;
                for plan in plans {
                    let evaluation = self.evaluate_grep_plan(plan)?;
                    decoded_posting_count = decoded_posting_count
                        .checked_add(evaluation.decoded_posting_count)
                        .ok_or_else(|| "resident GREP decoded posting count overflows".to_owned())?;
                    if let Some(value) = evaluation.smallest_posting_count {
                        smallest_posting_count = Some(
                            smallest_posting_count
                                .map_or(value, |smallest: usize| smallest.min(value)),
                        );
                    }
                    union = union_sorted(&union, &evaluation.owners);
                }
                Ok(PlanEvaluation {
                    owners: union,
                    decoded_posting_count,
                    smallest_posting_count,
                })
            }
        }
    }
}

struct PlanEvaluation {
    owners: Vec<u32>,
    decoded_posting_count: usize,
    smallest_posting_count: Option<usize>,
}

fn sum_decoded_postings(evaluations: &[PlanEvaluation]) -> Result<usize, String> {
    evaluations.iter().try_fold(0usize, |total, evaluation| {
        total
            .checked_add(evaluation.decoded_posting_count)
            .ok_or_else(|| "resident GREP decoded posting count overflows".to_owned())
    })
}

fn smallest_posting(evaluations: &[PlanEvaluation]) -> Option<usize> {
    evaluations
        .iter()
        .filter_map(|evaluation| evaluation.smallest_posting_count)
        .min()
}

fn intersect_postings(mut postings: Vec<Vec<u32>>) -> Result<Vec<u32>, String> {
    postings.sort_unstable_by_key(Vec::len);
    let mut posting_iter = postings.into_iter();
    let Some(mut intersection) = posting_iter.next() else {
        return Err("resident GREP candidate plan contains no postings".to_owned());
    };
    for posting in posting_iter {
        intersection = intersect_sorted(&intersection, &posting);
        if intersection.is_empty() {
            break;
        }
    }
    Ok(intersection)
}

fn union_sorted(left: &[u32], right: &[u32]) -> Vec<u32> {
    let mut union = Vec::with_capacity(left.len().saturating_add(right.len()));
    let (mut left_index, mut right_index) = (0usize, 0usize);
    while left_index < left.len() && right_index < right.len() {
        match left[left_index].cmp(&right[right_index]) {
            std::cmp::Ordering::Less => {
                union.push(left[left_index]);
                left_index += 1;
            }
            std::cmp::Ordering::Greater => {
                union.push(right[right_index]);
                right_index += 1;
            }
            std::cmp::Ordering::Equal => {
                union.push(left[left_index]);
                left_index += 1;
                right_index += 1;
            }
        }
    }
    union.extend_from_slice(&left[left_index..]);
    union.extend_from_slice(&right[right_index..]);
    union
}

fn intersect_sorted(left: &[u32], right: &[u32]) -> Vec<u32> {
    let mut intersection = Vec::with_capacity(left.len().min(right.len()));
    let (mut left_index, mut right_index) = (0usize, 0usize);
    while left_index < left.len() && right_index < right.len() {
        match left[left_index].cmp(&right[right_index]) {
            std::cmp::Ordering::Less => left_index += 1,
            std::cmp::Ordering::Greater => right_index += 1,
            std::cmp::Ordering::Equal => {
                intersection.push(left[left_index]);
                left_index += 1;
                right_index += 1;
            }
        }
    }
    intersection
}

fn elapsed_nanos(started: std::time::Instant) -> u64 {
    started.elapsed().as_nanos().try_into().unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "../tests/unit/resident_byte_coverage.rs"]
mod tests;
