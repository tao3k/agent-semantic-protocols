// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeSet;

use crate::ranked_text_selector_candidates;

fn hit(
    owner: &str,
    language: &str,
    symbol: &str,
) -> agent_semantic_symbol_index::SymbolSkeletonHitV1 {
    agent_semantic_symbol_index::SymbolSkeletonHitV1 {
        owner_path: owner.to_owned(),
        owner_content_digest: format!("blake3-256:{}", "0".repeat(64)),
        language_id: Some(language.to_owned()),
        structural_selector: Some(format!("{language}://{owner}#item/function/{symbol}")),
        matched_keys: vec![symbol.to_owned()],
    }
}

#[test]
fn owner_scope_never_expands_unproven_sibling_selectors() {
    let owner_scope = ["src/lib.rs".to_owned()].into_iter().collect();
    let languages = ["rust"].into_iter().collect();
    let proven = hit("src/lib.rs", "rust", "proven");
    let candidates = ranked_text_selector_candidates(
        "title:lib OR body:proven",
        &owner_scope,
        &languages,
        [proven],
        8,
    )
    .expect("project exact selector carrier");
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].selector,
        "rust://src/lib.rs#item/function/proven"
    );

    assert!(
        ranked_text_selector_candidates(
            "title:lib OR body:missing",
            &owner_scope,
            &languages,
            [],
            8,
        )
        .expect("owner-only hit is not an exact selector")
        .is_empty()
    );
}

#[test]
fn carrier_rejects_cross_owner_and_cross_language_hits() {
    let owner_scope = ["src/lib.rs".to_owned()].into_iter().collect();
    let languages = ["rust"].into_iter().collect();
    assert!(
        ranked_text_selector_candidates(
            "body:hidden",
            &owner_scope,
            &languages,
            [
                hit("src/other.rs", "rust", "hidden"),
                hit("src/lib.rs", "python", "hidden"),
            ],
            8,
        )
        .expect("filter non-admitted hits")
        .is_empty()
    );
}

#[test]
fn carrier_budget_fails_closed_without_partial_success() {
    let owner_scope = ["src/lib.rs".to_owned()]
        .into_iter()
        .collect::<BTreeSet<_>>();
    let languages = ["rust"].into_iter().collect();
    let hits = ["first", "second"].map(|symbol| hit("src/lib.rs", "rust", symbol));
    let error = ranked_text_selector_candidates(
        "body:first OR body:second",
        &owner_scope,
        &languages,
        hits,
        1,
    )
    .expect_err("over-budget carrier must fail closed");
    assert!(error.contains("selector carrier budget exceeded"));
}

#[test]
fn selector_carrier_scenario_records_submillisecond_top_k_projection() {
    let scenario = toml::from_str::<toml::Value>(include_str!(
        "scenarios/ranked_text_selector_carrier/scenario.toml"
    ))
    .expect("ranked-text selector carrier Scenario");
    let benchmark = toml::from_str::<toml::Value>(include_str!(
        "scenarios/ranked_text_selector_carrier/benchmark.toml"
    ))
    .expect("ranked-text selector carrier benchmark receipt");
    let scheme = scenario["query"]["scheme"]
        .as_str()
        .expect("Scenario Scheme query");
    crate::parse_progressive_search_playbook_args(&[
        "search".to_owned(),
        "playbook".to_owned(),
        scheme.to_owned(),
    ])
    .expect("Scenario uses the admitted V1 Scheme Search grammar");
    let candidate_count = scenario["model"]["candidate_count"]
        .as_integer()
        .and_then(|value| usize::try_from(value).ok())
        .expect("candidate count");
    let samples = scenario["benchmark"]["samples"]
        .as_integer()
        .and_then(|value| usize::try_from(value).ok())
        .expect("sample count");
    let p99_limit = scenario["benchmark"]["maximum_p99_nanos"]
        .as_integer()
        .and_then(|value| u128::try_from(value).ok())
        .expect("p99 limit");
    assert_eq!(
        benchmark["qualification"]["maximum_p99_nanos"].as_integer(),
        Some(i64::try_from(p99_limit).expect("p99 limit fits i64"))
    );
    assert_eq!(
        benchmark["observed"]["unproven_sibling_results"].as_integer(),
        Some(0)
    );
    let owner_scope = ["src/lib.rs".to_owned()].into_iter().collect();
    let languages = ["rust"].into_iter().collect();
    let hits = (0..candidate_count)
        .map(|index| hit("src/lib.rs", "rust", &format!("symbol_{index}")))
        .collect::<Vec<_>>();
    let mut observations = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = std::time::Instant::now();
        let projected = ranked_text_selector_candidates(
            "body:symbol",
            &owner_scope,
            &languages,
            hits.clone(),
            candidate_count,
        )
        .expect("project bounded selector carrier");
        observations.push(started.elapsed().as_nanos());
        assert_eq!(projected.len(), candidate_count);
    }
    observations.sort_unstable();
    let p99 = observations[(observations.len() * 99 / 100).min(observations.len() - 1)];
    eprintln!(
        "ranked-text-selector-carrier candidateCount={candidate_count} samples={samples} p99Nanos={p99}"
    );
    assert!(p99 < p99_limit, "selector carrier p99={p99}ns");
}
