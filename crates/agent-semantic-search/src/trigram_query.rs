// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Russ Cox / `microsoft/tgrep`-inspired `regex-syntax` HIR to Boolean Trigram
//! Query Plan compiler.
//!
//! Compiles arbitrary regex patterns into a sound (zero-false-negative) boolean
//! expression over ASCII-lowercased literals (`All | None | Literal | And | Or`).
//! Unlike single-literal-run heuristics, alternations such as `(foo_bar|baz_qux)`
//! compile directly to `Or([Literal("foo_bar"), Literal("baz_qux")])` instead of
//! falling back to linear scans, while preserving full soundness under Rust's
//! Unicode case-insensitive regex engine (including folds such as `s`/`ſ` and `k`/`K`).

use regex_syntax::ParserBuilder;
use regex_syntax::hir::{Class, Hir, HirKind};

const MAX_EXACT_VARIANTS: usize = 24;
const MAX_EXACT_BYTES: usize = 128;
const MAX_CLASS_FOLDED_VARIANTS: usize = 8;

/// Sound boolean trigram query plan evaluated against [`crate::trigram_store::FlatTrigramStore`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrigramQuery {
    /// Safe relaxation: any live file may match.
    All,
    /// Provably unsatisfiable (empty character class / empty alternation).
    None,
    /// Mandatory contiguous ASCII-lowercased byte run (length >= 3) that will be
    /// evaluated with both `loc_mask` adjacency rotation and `next_mask` 3.5-gram filtering.
    Literal(Vec<u8>),
    /// Every sub-query must match the candidate file.
    And(Vec<TrigramQuery>),
    /// At least one sub-query must match the candidate file.
    Or(Vec<TrigramQuery>),
}

impl TrigramQuery {
    /// Combine sub-queries with logical AND and algebraic simplification.
    #[must_use]
    pub fn and(items: Vec<Self>) -> Self {
        let mut flat = Vec::new();
        for item in items {
            match item {
                Self::None => return Self::None,
                Self::All => {}
                Self::And(inner) => flat.extend(inner),
                other => flat.push(other),
            }
        }
        dedup_queries(&mut flat);
        match flat.len() {
            0 => Self::All,
            1 => flat.pop().unwrap_or(Self::All),
            _ => Self::And(flat),
        }
    }

    /// Combine sub-queries with logical OR and algebraic simplification.
    #[must_use]
    pub fn or(items: Vec<Self>) -> Self {
        let mut flat = Vec::new();
        for item in items {
            match item {
                Self::All => return Self::All,
                Self::None => {}
                Self::Or(inner) => flat.extend(inner),
                other => flat.push(other),
            }
        }
        dedup_queries(&mut flat);
        match flat.len() {
            0 => Self::None,
            1 => flat.pop().unwrap_or(Self::None),
            _ => Self::Or(flat),
        }
    }
}

fn dedup_queries(queries: &mut Vec<TrigramQuery>) {
    let mut out = Vec::with_capacity(queries.len());
    for q in queries.drain(..) {
        if !out.contains(&q) {
            out.push(q);
        }
    }
    *queries = out;
}

/// Compile a regex pattern (using the exact `case_insensitive` setting passed to
/// `regex::RegexBuilder`) into a zero-false-negative [`TrigramQuery`].
///
/// If parsing fails, safely relaxes to [`TrigramQuery::All`].
#[must_use]
pub fn compile_regex_trigram_query(pattern: &str, case_insensitive: bool) -> TrigramQuery {
    let Ok(hir) = ParserBuilder::new()
        .case_insensitive(case_insensitive)
        .build()
        .parse(pattern)
    else {
        return TrigramQuery::All;
    };
    compile_hir(&hir).into_query()
}

/// Intermediate analysis of an HIR sub-expression:
/// - `exact`: if `Some(variants)`, the sub-expression matches *only* byte strings
///   whose ASCII-lowercased form is one of `variants` (small bounded set).
///   Preserving `exact` across `Concat` enables cross-boundary trigram extraction
///   (e.g. `(foo|bar)_baz` -> `Or([Literal("foo_baz"), Literal("bar_baz")])`).
/// - `query`: sound fallback [`TrigramQuery`] whenever `exact` is `None`.
#[derive(Clone, Debug)]
struct HirAnalysis {
    exact: Option<Vec<Vec<u8>>>,
    query: TrigramQuery,
}

impl HirAnalysis {
    fn all() -> Self {
        Self {
            exact: None,
            query: TrigramQuery::All,
        }
    }

    fn none() -> Self {
        Self {
            exact: Some(Vec::new()),
            query: TrigramQuery::None,
        }
    }

    fn empty_match() -> Self {
        Self {
            exact: Some(vec![Vec::new()]),
            query: TrigramQuery::All,
        }
    }

    fn from_exact(mut variants: Vec<Vec<u8>>) -> Self {
        variants.sort_unstable();
        variants.dedup();
        if variants.is_empty() {
            return Self::none();
        }
        let query = exact_variants_to_query(&variants);
        Self {
            exact: Some(variants),
            query,
        }
    }

    fn into_query(self) -> TrigramQuery {
        match self.exact {
            Some(variants) => exact_variants_to_query(&variants),
            None => self.query,
        }
    }
}

fn exact_variants_to_query(variants: &[Vec<u8>]) -> TrigramQuery {
    if variants.is_empty() {
        return TrigramQuery::None;
    }
    let mut branches = Vec::with_capacity(variants.len());
    for variant in variants {
        if variant.len() < 3 {
            // If any allowed match has < 3 bytes, no trigram is mandatory across all matches.
            return TrigramQuery::All;
        }
        branches.push(TrigramQuery::Literal(variant.clone()));
    }
    TrigramQuery::or(branches)
}

fn ascii_lower_vec(bytes: &[u8]) -> Vec<u8> {
    bytes.iter().map(u8::to_ascii_lowercase).collect()
}

fn compile_hir(hir: &Hir) -> HirAnalysis {
    match hir.kind() {
        HirKind::Empty | HirKind::Look(_) => HirAnalysis::empty_match(),
        HirKind::Literal(lit) => HirAnalysis::from_exact(vec![ascii_lower_vec(&lit.0)]),
        HirKind::Class(class) => compile_class(class),
        HirKind::Capture(cap) => compile_hir(&cap.sub),
        HirKind::Repetition(rep) => compile_repetition(rep.min, rep.max, &rep.sub),
        HirKind::Concat(subs) => compile_concat(subs),
        HirKind::Alternation(subs) => compile_alternation(subs),
    }
}

fn compile_class(class: &Class) -> HirAnalysis {
    let mut variants: Vec<Vec<u8>> = Vec::new();
    match class {
        Class::Unicode(uc) => {
            if uc.ranges().is_empty() {
                return HirAnalysis::none();
            }
            let mut utf8_buf = [0u8; 4];
            for range in uc.iter() {
                for ch in range.start()..=range.end() {
                    let encoded = ch.encode_utf8(&mut utf8_buf);
                    let lowered = ascii_lower_vec(encoded.as_bytes());
                    if !variants.contains(&lowered) {
                        variants.push(lowered);
                        if variants.len() > MAX_CLASS_FOLDED_VARIANTS {
                            return HirAnalysis::all();
                        }
                    }
                }
            }
        }
        Class::Bytes(bc) => {
            if bc.ranges().is_empty() {
                return HirAnalysis::none();
            }
            for range in bc.iter() {
                for b in range.start()..=range.end() {
                    let lowered = vec![b.to_ascii_lowercase()];
                    if !variants.contains(&lowered) {
                        variants.push(lowered);
                        if variants.len() > MAX_CLASS_FOLDED_VARIANTS {
                            return HirAnalysis::all();
                        }
                    }
                }
            }
        }
    }
    HirAnalysis::from_exact(variants)
}

fn compile_repetition(min: u32, max: Option<u32>, sub: &Hir) -> HirAnalysis {
    if min == 0 {
        // Optional repetition (`*`, `?`, `{0,n}`) may match empty -> safe relaxation.
        return HirAnalysis::all();
    }
    let inner = compile_hir(sub);
    if max == Some(min)
        && min <= 8
        && let Some(ref base_variants) = inner.exact
    {
        if base_variants.is_empty() {
            return HirAnalysis::none();
        }
        let mut acc = vec![Vec::new()];
        let mut ok = true;
        for _ in 0..min {
            match cross_concat(&acc, base_variants) {
                Some(next) => acc = next,
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            return HirAnalysis::from_exact(acc);
        }
    }
    // `sub` must occur at least once (`+`, `{min,max}` with min >= 1), so its query is mandatory.
    HirAnalysis {
        exact: None,
        query: inner.into_query(),
    }
}

fn compile_concat(subs: &[Hir]) -> HirAnalysis {
    if subs.is_empty() {
        return HirAnalysis::empty_match();
    }
    let mut rolling_exact: Option<Vec<Vec<u8>>> = Some(vec![Vec::new()]);
    let mut and_terms: Vec<TrigramQuery> = Vec::new();
    let mut all_children_exact = true;

    for sub in subs {
        let child = compile_hir(sub);
        if matches!(child.query, TrigramQuery::None)
            || child.exact.as_ref().is_some_and(Vec::is_empty)
        {
            return HirAnalysis::none();
        }
        match (&mut rolling_exact, child.exact) {
            (Some(acc), Some(child_variants)) => {
                if let Some(combined) = cross_concat(acc, &child_variants) {
                    *acc = combined;
                } else {
                    all_children_exact = false;
                    and_terms.push(exact_variants_to_query(acc));
                    and_terms.push(exact_variants_to_query(&child_variants));
                    rolling_exact = None;
                }
            }
            (Some(acc), None) => {
                all_children_exact = false;
                and_terms.push(exact_variants_to_query(acc));
                and_terms.push(child.query);
                rolling_exact = None;
            }
            (None, Some(child_variants)) => {
                rolling_exact = Some(child_variants);
            }
            (None, None) => {
                and_terms.push(child.query);
            }
        }
    }

    if all_children_exact && let Some(exact) = rolling_exact {
        return HirAnalysis::from_exact(exact);
    }
    if let Some(tail) = rolling_exact {
        and_terms.push(exact_variants_to_query(&tail));
    }
    HirAnalysis {
        exact: None,
        query: TrigramQuery::and(and_terms),
    }
}

fn compile_alternation(subs: &[Hir]) -> HirAnalysis {
    if subs.is_empty() {
        return HirAnalysis::none();
    }
    let mut exact_union: Option<Vec<Vec<u8>>> = Some(Vec::new());
    let mut or_terms: Vec<TrigramQuery> = Vec::with_capacity(subs.len());

    for sub in subs {
        let branch = compile_hir(sub);
        match (&mut exact_union, &branch.exact) {
            (Some(acc), Some(vars)) => {
                for v in vars {
                    if !acc.contains(v) {
                        acc.push(v.clone());
                    }
                }
                if acc.len() > MAX_EXACT_VARIANTS {
                    exact_union = None;
                }
            }
            _ => {
                exact_union = None;
            }
        }
        or_terms.push(branch.into_query());
    }

    if let Some(exact) = exact_union {
        return HirAnalysis::from_exact(exact);
    }
    HirAnalysis {
        exact: None,
        query: TrigramQuery::or(or_terms),
    }
}

fn cross_concat(left: &[Vec<u8>], right: &[Vec<u8>]) -> Option<Vec<Vec<u8>>> {
    if left.len().saturating_mul(right.len()) > MAX_EXACT_VARIANTS {
        return None;
    }
    let mut out = Vec::with_capacity(left.len() * right.len());
    for l in left {
        for r in right {
            if l.len() + r.len() > MAX_EXACT_BYTES {
                return None;
            }
            let mut combined = Vec::with_capacity(l.len() + r.len());
            combined.extend_from_slice(l);
            combined.extend_from_slice(r);
            if !out.contains(&combined) {
                out.push(combined);
                if out.len() > MAX_EXACT_VARIANTS {
                    return None;
                }
            }
        }
    }
    Some(out)
}
