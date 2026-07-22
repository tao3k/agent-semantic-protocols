use std::collections::BTreeSet;

#[path = "support/match_config.rs"]
mod match_config;

#[test]
fn real_hook_config_drives_every_match_command_scenario() {
    assert!(match_config::wrapper_match_enabled());
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
fn wrapped_cargo_test_arguments_cannot_match_raw_search_rules() {
    let command = "timeout 30s direnv exec . cargo test -p agent-semantic-protocol \
        --test unit_test codex_hook_auto_syncs_stale_managed_matcher_contract -- --nocapture";
    let cases = match_config::rule_prefixes();
    let mut raw_search_prefixes = 0usize;
    let mut testing_lane_prefixes = 0usize;

    for case in &cases {
        if case.rule_id == "deny-uncontrolled-source-search-commands" {
            raw_search_prefixes += 1;
            assert_eq!(
                match_config::outcome(case, command),
                match_config::outcome(case, "asp-command-match-negative-control"),
                "argument tokens escaped into executable matching: prefix={:?}",
                case.argv_prefix
            );
        }
        if case.rule_id == "resident-testing-dispatch"
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
        raw_search_prefixes > 0,
        "raw search rule missing from config"
    );
    assert!(
        testing_lane_prefixes > 0,
        "resident testing dispatch missing from config"
    );
}
