// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::io::Write;
use std::sync::Arc;

use super::{ResidentByteCoverageIndex, ResidentByteCoverageInput};
use crate::{ResidentGrepCandidatePlan, build_resident_grep_candidate_plan};

fn owners<'a>(
    values: &'a [(&'a str, &'a [u8])],
) -> impl Iterator<Item = ResidentByteCoverageInput<'a>> {
    values
        .iter()
        .map(|(owner_path, bytes)| ResidentByteCoverageInput {
            owner_path: (*owner_path).to_owned(),
            authority: None,
            bytes,
        })
}

#[test]
fn hir_conjunction_intersects_before_exact_matching() {
    let values = [
        ("src/both.rs", b"Run any Endpoint".as_slice()),
        ("src/run.rs", b"Run only".as_slice()),
        ("src/endpoint.rs", b"Endpoint only".as_slice()),
    ];
    let index = ResidentByteCoverageIndex::new(owners(&values)).unwrap();
    let plan = build_resident_grep_candidate_plan("Run.*Endpoint", false, true).unwrap();
    assert_eq!(
        index
            .candidate_owner_paths_for_grep_plan(&plan, None, 16)
            .unwrap(),
        ["src/both.rs"]
    );
}

#[test]
fn hir_alternation_unions_sorted_unique_owner_ids() {
    let values = [
        ("src/alpha.rs", b"alpha_key".as_slice()),
        ("src/both.rs", b"alpha_key beta_key".as_slice()),
        ("src/beta.rs", b"beta_key".as_slice()),
    ];
    let index = ResidentByteCoverageIndex::new(owners(&values)).unwrap();
    let plan = build_resident_grep_candidate_plan("alpha_key|beta_key", false, true).unwrap();
    assert_eq!(
        index
            .candidate_owner_paths_for_grep_plan(&plan, None, 16)
            .unwrap(),
        ["src/alpha.rs", "src/beta.rs", "src/both.rs"]
    );
}

#[test]
fn match_all_never_becomes_an_empty_absence_claim() {
    let values = [("src/lib.rs", b"anything".as_slice())];
    let index = ResidentByteCoverageIndex::new(owners(&values)).unwrap();
    let error = index
        .candidate_owner_paths_for_grep_plan(&ResidentGrepCandidatePlan::MatchAll, None, 16)
        .expect_err("unbounded candidate plan must fail typed");
    assert!(error.contains("reasonKind=resident-rg-pattern-not-materialized"));
}

#[test]
fn mapped_artifact_keeps_postings_off_the_rust_heap() {
    let values = [
        ("src/a.rs", b"alpha_key".as_slice()),
        ("src/b.rs", b"beta_key".as_slice()),
    ];
    let (owners, artifact) = ResidentByteCoverageIndex::encode_artifact(owners(&values)).unwrap();
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(&artifact).unwrap();
    file.flush().unwrap();
    let mapping = Arc::new(unsafe { memmap2::MmapOptions::new().map(&file).unwrap() });
    let index = ResidentByteCoverageIndex::from_mapped_artifact(
        owners,
        Arc::clone(&mapping),
        0..mapping.len(),
    )
    .unwrap();
    assert_eq!(index.stats().artifact_heap_bytes, 0);
    assert_eq!(index.stats().owner_count, 2);
    assert!(index.stats().posting_count > 0);
}

#[test]
fn corrupt_artifact_is_rejected_before_query() {
    let values = [("src/lib.rs", b"alpha_key".as_slice())];
    let (owners, mut artifact) =
        ResidentByteCoverageIndex::encode_artifact(owners(&values)).unwrap();
    let last = artifact.len() - 1;
    artifact[last] ^= 1;
    assert!(ResidentByteCoverageIndex::from_owned_artifact(owners, artifact).is_err());
}

#[test]
fn exact_regex_hits_are_always_a_subset_of_trigram_candidates() {
    let values = [
        ("src/alpha.rs", b"alpha_key and gamma_end".as_slice()),
        ("src/beta.rs", b"beta_key then Endpoint".as_slice()),
        ("src/run.rs", b"Run through the Endpoint".as_slice()),
        ("src/repeat.rs", b"abcabc tail".as_slice()),
        ("src/unrelated.rs", b"nothing relevant".as_slice()),
    ];
    let index = ResidentByteCoverageIndex::new(owners(&values)).unwrap();
    for expression in [
        "alpha_key",
        "Run.*Endpoint",
        "alpha_key|beta_key",
        "(?:alpha|beta)_key",
        "(?:abc){2}",
        "(?:alpha_key)?gamma_end",
    ] {
        let plan = build_resident_grep_candidate_plan(expression, false, true).unwrap();
        assert!(
            !plan.is_match_all(),
            "fixture must exercise a selective plan: {expression}"
        );
        let candidates = index
            .candidate_owner_paths_for_grep_plan(&plan, None, values.len())
            .unwrap()
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        let exact = regex::bytes::RegexBuilder::new(expression)
            .unicode(true)
            .build()
            .unwrap();
        for (owner_path, bytes) in values {
            if exact.is_match(bytes) {
                assert!(
                    candidates.contains(owner_path),
                    "candidate plan dropped exact hit {owner_path} for {expression}: {plan:?}"
                );
            }
        }
    }
}

#[test]
fn unicode_case_folding_fails_open_instead_of_repeating_tgrep_false_negative() {
    let values = [
        ("src/upper.txt", "ÜBER".as_bytes()),
        ("src/lower.txt", "über".as_bytes()),
    ];
    let index = ResidentByteCoverageIndex::new(owners(&values)).unwrap();
    let plan = build_resident_grep_candidate_plan("über", true, true).unwrap();
    assert_eq!(plan, ResidentGrepCandidatePlan::MatchAll);
    let error = index
        .candidate_owner_paths_for_grep_plan(&plan, None, values.len())
        .expect_err("non-sound Unicode case-folding plan must not filter owners");
    assert!(error.contains("reasonKind=resident-rg-pattern-not-materialized"));
}

#[test]
fn rarest_first_stops_decoding_after_candidate_intersection_becomes_empty() {
    let values = [
        ("src/left.txt", b"abcd".as_slice()),
        ("src/right.txt", b"cdef".as_slice()),
    ];
    let index = ResidentByteCoverageIndex::new(owners(&values)).unwrap();
    let plan = build_resident_grep_candidate_plan("abcdef", false, true).unwrap();
    let (candidates, receipt) = index
        .candidate_owner_paths_for_grep_plan_with_receipt(&plan, None, values.len())
        .unwrap();
    assert!(candidates.is_empty());
    assert_eq!(receipt.requested_gram_count, 4);
    assert_eq!(receipt.decoded_posting_count, 3);
}
