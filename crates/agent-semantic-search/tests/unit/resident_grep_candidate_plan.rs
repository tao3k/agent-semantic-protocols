// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{ResidentGrepCandidatePlan, build_resident_grep_candidate_plan};

#[test]
fn concat_retains_literals_on_both_sides_of_wildcard() {
    let plan = build_resident_grep_candidate_plan("Run.*Endpoint", false, true).unwrap();
    assert!(matches!(plan, ResidentGrepCandidatePlan::And(_)));
    assert_eq!(plan.distinct_gram_count(), 7);
}

#[test]
fn nested_alternation_remains_part_of_conjunction() {
    let plan = build_resident_grep_candidate_plan("abc(def|ghi)", false, true).unwrap();
    let ResidentGrepCandidatePlan::And(parts) = plan else {
        panic!("expected AND plan");
    };
    assert!(
        parts
            .iter()
            .any(|part| matches!(part, ResidentGrepCandidatePlan::Or(_)))
    );
}

#[test]
fn optional_or_short_only_expression_fails_open_to_match_all() {
    for expression in [".*", "ab", "(foo)?", "foo|"] {
        assert!(
            build_resident_grep_candidate_plan(expression, false, true)
                .unwrap()
                .is_match_all(),
            "{expression} must not produce a narrowing plan"
        );
    }
}

#[test]
fn unicode_case_folding_is_not_narrowed_without_a_proved_class_plan() {
    let plan = build_resident_grep_candidate_plan("über", true, true).unwrap();
    assert!(plan.is_match_all());
}
