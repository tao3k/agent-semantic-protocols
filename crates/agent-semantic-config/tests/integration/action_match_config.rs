// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_config::HookClientConfigFile;
use agent_semantic_config::HookClientMatcherPolicy;

fn default_config() -> HookClientConfigFile {
    agent_semantic_config::default_hook_client_config_file()
        .expect("generated default hook config should parse")
}

#[test]
fn command_action_catalog_is_a_compact_deterministic_argv_list() {
    let config = default_config();
    let patterns = config
        .command_action_patterns
        .iter()
        .flat_map(|family| {
            family
                .argv_pattern_any
                .iter()
                .map(move |pattern| (family.action, pattern))
        })
        .collect::<Vec<_>>();
    assert!(
        patterns
            .iter()
            .any(|(_, pattern)| *pattern == &vec!["head".to_owned()])
    );
    assert!(
        patterns
            .iter()
            .any(|(_, pattern)| *pattern == &vec!["rg".to_owned()])
    );
    assert!(patterns.iter().all(|(_, pattern)| {
        !pattern.is_empty()
            && !pattern[0].contains('/')
            && pattern.iter().all(|token| !token.is_empty())
    }));
    let unique = patterns.iter().collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(), patterns.len());
}

#[test]
fn command_action_catalog_rejects_duplicate_empty_and_path_executables() {
    let mut duplicate = default_config();
    duplicate.command_action_patterns[0]
        .argv_pattern_any
        .push(vec!["head".to_owned()]);
    assert!(
        duplicate
            .validate()
            .expect_err("duplicate Reader pattern")
            .contains("duplicate action/pattern")
    );

    let mut empty = default_config();
    empty.command_action_patterns[0]
        .argv_pattern_any
        .push(Vec::new());
    assert!(
        empty
            .validate()
            .expect_err("empty Reader pattern")
            .contains("must contain an executable basename")
    );

    let mut path = default_config();
    path.command_action_patterns[0]
        .argv_pattern_any
        .push(vec!["/usr/bin/head".to_owned()]);
    assert!(
        path.validate()
            .expect_err("path Reader pattern")
            .contains("must be an executable basename")
    );
}

#[test]
fn source_access_rule_uses_read_default_wrapping_and_language_profiles() {
    let config = default_config();
    let rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("Bash source access plus language profiles route should exist");
    let dispatch = rule
        .dispatch
        .as_ref()
        .expect("source access route dispatch");
    assert_eq!(dispatch.agent.as_str(), "asp_explorer");
    assert_eq!(rule.matcher.as_deref(), Some("Bash"));
    assert!(rule.matcher_policies.is_empty());
    assert!(!rule.profiles_list.is_empty());
    assert_eq!(
        config.agent_calling.symbol("codex", "asp_explorer"),
        "@asp_explorer"
    );
    assert_eq!(
        config.agent_calling.symbol("claude", "asp_explorer"),
        "@agent-asp-explorer"
    );
}

#[test]
fn default_template_uses_rule_local_matcher_policies() {
    let config = default_config();

    let wrapped_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "testing-role-dispatch")
        .expect("wrapped command rule");
    assert_eq!(
        wrapped_rule.matcher_policies,
        [HookClientMatcherPolicy::WrappedCommand]
    );
    let source_access_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("Bash source-access route rule");
    assert!(source_access_rule.matcher_policies.is_empty());
    assert_eq!(source_access_rule.matcher.as_deref(), Some("Bash"));
    assert!(
        source_access_rule
            .profiles_list
            .contains(&"rust".to_owned())
    );
    assert!(
        source_access_rule
            .profiles_list
            .contains(&"typescript".to_owned())
    );
}

#[test]
fn repository_git_history_uses_a_language_independent_command_set() {
    let config = default_config();
    let command_set = config
        .command_sets
        .iter()
        .find(|command_set| command_set.id == "git-history-inspection")
        .expect("Git history command set");
    assert!(
        command_set
            .argv_prefix_any
            .contains(&vec!["git".to_owned(), "show".to_owned()])
    );
    assert!(
        command_set
            .argv_prefix_any
            .contains(&vec!["git".to_owned(), "diff".to_owned()])
    );
    assert!(
        !command_set
            .argv_prefix_any
            .contains(&vec!["git".to_owned(), "ls-files".to_owned()])
    );

    let rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "git-history-inspection-dispatch")
        .expect("Git history Testing route");
    assert_eq!(
        rule.match_config.command_set_any,
        ["git-history-inspection"]
    );
    assert_eq!(
        rule.dispatch
            .as_ref()
            .map(|dispatch| dispatch.agent.as_str()),
        Some("asp_testing")
    );
}

#[test]
fn review_categories_have_an_explicit_testing_lane_intent() {
    let config = default_config();
    let rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "review-role-dispatch")
        .expect("review dispatch");
    assert_eq!(rule.intent.as_deref(), Some("review-command"));
    assert_eq!(
        rule.dispatch
            .as_ref()
            .map(|dispatch| dispatch.agent.as_str()),
        Some("asp_testing")
    );
    assert!(
        rule.match_config
            .command_profile_any
            .iter()
            .any(|reference| {
                reference.profile == "rust-cargo" && reference.category == "review"
            })
    );
}

#[test]
fn rust_format_review_requires_the_non_mutating_check_token() {
    let config = default_config();
    let rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "rust-format-check-role-dispatch")
        .expect("Rust format check dispatch");
    assert_eq!(rule.intent.as_deref(), Some("review-command"));
    assert_eq!(rule.match_config.argv_token_all, ["--check"]);
    assert!(
        rule.match_config
            .command_profile_any
            .iter()
            .any(|reference| {
                reference.profile == "rust-cargo" && reference.category == "format-check"
            })
    );
}

#[test]
fn matcher_policy_rfc_records_parser_owned_snapshot_contract() {
    let rfc = include_str!(
        "../../../../docs/10-19-rfcs/10.15-agent-hook-interception-protocol/10.15.40-parser-owned-wrapper-match-snapshots.org"
    );

    assert!(rfc.contains("matcherPolicies"));
    assert!(rfc.contains("wrapped_command"));
    assert!(rfc.contains("commandWrappers"));
    assert!(rfc.contains("Git snapshot"));
}

#[test]
fn typed_action_rule_shape_probe() {
    let config = default_config();
    let rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("typed action rule should exist");
    eprintln!("{rule:#?}");
}
