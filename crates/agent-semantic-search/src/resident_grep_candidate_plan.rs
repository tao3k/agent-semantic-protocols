// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Sound regex-HIR to trigram candidate planning for resident GREP.
//!
//! A plan may over-select owners, but it must never remove an owner that the
//! exact resident matcher can accept. Unsupported or insufficiently selective
//! HIR nodes therefore become `MatchAll`, never an empty result.

use regex_syntax::hir::{Hir, HirKind, Literal};

use crate::RESIDENT_BYTE_GRAM_WIDTH;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResidentGrepCandidatePlan {
    MatchAll,
    Grams(Vec<u32>),
    And(Vec<Self>),
    Or(Vec<Self>),
}

impl ResidentGrepCandidatePlan {
    #[must_use]
    pub fn is_match_all(&self) -> bool {
        matches!(self, Self::MatchAll)
    }

    #[must_use]
    pub fn distinct_gram_count(&self) -> usize {
        let mut grams = std::collections::BTreeSet::new();
        self.collect_grams(&mut grams);
        grams.len()
    }

    fn collect_grams(&self, grams: &mut std::collections::BTreeSet<u32>) {
        match self {
            Self::MatchAll => {}
            Self::Grams(values) => grams.extend(values),
            Self::And(plans) | Self::Or(plans) => {
                for plan in plans {
                    plan.collect_grams(grams);
                }
            }
        }
    }
}

pub fn build_resident_grep_candidate_plan(
    expression: &str,
    case_insensitive: bool,
    unicode: bool,
) -> Result<ResidentGrepCandidatePlan, String> {
    let hir = regex_syntax::ParserBuilder::new()
        .case_insensitive(case_insensitive)
        .unicode(unicode)
        .utf8(false)
        .build()
        .parse(expression)
        .map_err(|error| format!("resident GREP candidate-plan parse failed: {error}"))?;
    Ok(plan_hir(&hir))
}

fn plan_hir(hir: &Hir) -> ResidentGrepCandidatePlan {
    match hir.kind() {
        HirKind::Literal(Literal(bytes)) => plan_literal(bytes),
        HirKind::Concat(parts) => simplify_and(parts.iter().map(plan_hir).collect()),
        HirKind::Alternation(branches) => simplify_or(branches.iter().map(plan_hir).collect()),
        HirKind::Repetition(repetition) if repetition.min >= 1 => plan_hir(&repetition.sub),
        HirKind::Capture(capture) => plan_hir(&capture.sub),
        HirKind::Repetition(_) | HirKind::Class(_) | HirKind::Look(_) | HirKind::Empty => {
            ResidentGrepCandidatePlan::MatchAll
        }
    }
}

fn plan_literal(bytes: &[u8]) -> ResidentGrepCandidatePlan {
    if bytes.len() < RESIDENT_BYTE_GRAM_WIDTH {
        return ResidentGrepCandidatePlan::MatchAll;
    }
    let grams = bytes
        .windows(RESIDENT_BYTE_GRAM_WIDTH)
        .map(|window| pack_gram(window[0], window[1], window[2]))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    ResidentGrepCandidatePlan::Grams(grams)
}

fn simplify_and(plans: Vec<ResidentGrepCandidatePlan>) -> ResidentGrepCandidatePlan {
    let mut retained = Vec::new();
    for plan in plans {
        match plan {
            ResidentGrepCandidatePlan::MatchAll => {}
            ResidentGrepCandidatePlan::And(nested) => retained.extend(nested),
            other => retained.push(other),
        }
    }
    match retained.len() {
        0 => ResidentGrepCandidatePlan::MatchAll,
        1 => retained.pop().expect("one retained AND plan"),
        _ => ResidentGrepCandidatePlan::And(retained),
    }
}

fn simplify_or(plans: Vec<ResidentGrepCandidatePlan>) -> ResidentGrepCandidatePlan {
    let mut retained = Vec::new();
    for plan in plans {
        match plan {
            ResidentGrepCandidatePlan::MatchAll => return ResidentGrepCandidatePlan::MatchAll,
            ResidentGrepCandidatePlan::Or(nested) => retained.extend(nested),
            other => retained.push(other),
        }
    }
    match retained.len() {
        0 => ResidentGrepCandidatePlan::MatchAll,
        1 => retained.pop().expect("one retained OR plan"),
        _ => ResidentGrepCandidatePlan::Or(retained),
    }
}

fn pack_gram(first: u8, second: u8, third: u8) -> u32 {
    (u32::from(first) << 16) | (u32::from(second) << 8) | u32::from(third)
}

#[cfg(test)]
#[path = "../tests/unit/resident_grep_candidate_plan.rs"]
mod tests;
