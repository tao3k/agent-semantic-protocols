// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeSet;

#[path = "../support/match_config.rs"]
mod match_config;

#[test]
fn real_hook_config_drives_every_match_command_scenario() {
    let cases = match_config::rule_prefixes();
    assert!(!cases.is_empty());
    assert!(cases.iter().all(|case| !case.argv_prefix.is_empty()));
    assert!(
        cases
            .iter()
            .map(|case| case.rule_id.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            > 1,
        "the contract must cover multiple real rules"
    );

    for case in cases {
        match_config::assert_case(&case);
    }
}

#[test]
fn wrapped_cargo_test_arguments_match_only_the_testing_lane() {
    let command = "timeout 30s direnv exec . cargo test -p agent-semantic-client \
        --test unit_test codex_hook_auto_syncs_stale_managed_matcher_contract -- --nocapture";
    let cases = match_config::rule_prefixes();
    let mut testing_lane_prefixes = 0usize;

    for case in &cases {
        if case.rule_id == "testing-role-dispatch"
            && case.argv_prefix == ["cargo".to_string(), "test".to_string()]
        {
            testing_lane_prefixes += 1;
            assert_eq!(
                match_config::outcome(case, command),
                match_config::outcome(case, &case.argv_prefix.join(" ")),
                "testing lane failed to unwrap timeout/direnv: prefix={:?}",
                case.argv_prefix
            );
        }
    }

    assert!(
        testing_lane_prefixes > 0,
        "resident testing dispatch missing from config"
    );
}

#[test]
fn wrapper_match_enable_accepts_arbitrary_wrapper_names() {
    let testing_case = match_config::rule_prefixes()
        .into_iter()
        .find(|case| {
            case.rule_id == "testing-role-dispatch"
                && case.wrapped_command
                && case.argv_prefix == ["cargo".to_string(), "test".to_string()]
        })
        .expect("cargo test dispatch rule");

    for command in [
        "direnv exec . cargo test -p agent-semantic-hook --lib runtime_profiles_for_",
        "organization-local-wrapper --profile ci cargo test -p agent-semantic-hook --lib runtime_profiles_for_",
        "future-wrapper alpha beta gamma cargo test -p agent-semantic-hook --lib runtime_profiles_for_",
    ] {
        assert_eq!(
            match_config::outcome(&testing_case, command),
            match_config::outcome(&testing_case, "cargo test"),
            "{command}"
        );
    }
}

#[test]
fn wrapped_command_match_stays_within_git_snapshot_budget() {
    let scenario = toml::from_str::<toml::Value>(include_str!(
        "../../../agent-semantic-hook/tests/fixtures/scenarios/generic_wrapper_testing_role_dispatch/scenario.toml"
    ))
    .expect("hook match scenario snapshot");
    let commands = scenario["commands"].as_array().expect("scenario commands");
    let max_matcher_micros = scenario["performance"]["maxMatcherMicros"]
        .as_integer()
        .expect("max matcher micros") as u128;
    let testing_case = match_config::rule_prefixes()
        .into_iter()
        .find(|case| {
            case.rule_id == "testing-role-dispatch"
                && case.argv_prefix == ["cargo".to_string(), "test".to_string()]
        })
        .expect("cargo test dispatch rule");

    let iterations = 512u128;
    let started = std::time::Instant::now();
    for _ in 0..iterations {
        for command in commands {
            assert!(matches!(
                match_config::outcome(&testing_case, command.as_str().expect("scenario command")),
                agent_semantic_shell_parser::BashCommandMatch::Parsed(
                    agent_semantic_shell_parser::PrefixMatch::Matched
                )
            ));
        }
    }
    let average_micros = started.elapsed().as_micros() / (iterations * commands.len() as u128);
    eprintln!(
        "[wrapper-match-benchmark] samples={} averageMicros={average_micros} budgetMicros={max_matcher_micros}",
        iterations * commands.len() as u128
    );
    assert!(
        average_micros <= max_matcher_micros,
        "average matcher latency {average_micros}us exceeded {max_matcher_micros}us"
    );
}
