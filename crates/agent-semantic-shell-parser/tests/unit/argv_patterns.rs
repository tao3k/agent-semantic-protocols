// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_shell_parser::command_tokens_match_argv_pattern;

fn words(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn argv_sequence_wildcard_accepts_zero_one_or_many_tokens() {
    let pattern = words(&["reader", "*"]);
    for actual in [
        words(&["reader"]),
        words(&["reader", "source.rs"]),
        words(&["reader", "--flag", "value", "source.rs"]),
    ] {
        assert!(command_tokens_match_argv_pattern(
            &actual,
            &pattern,
            false,
            "source.rs"
        ));
    }
}

#[test]
fn executable_basename_and_wrapped_candidates_share_one_matcher() {
    let pattern = words(&["reader", "--mode=*"]);
    assert!(command_tokens_match_argv_pattern(
        &words(&[
            "/usr/bin/env",
            "/opt/tools/reader",
            "--mode=compact",
            "source.rs",
        ]),
        &pattern,
        true,
        "source.rs",
    ));
}

#[test]
fn fixed_argv_globs_do_not_match_unrelated_tokens() {
    let pattern = words(&["reader", "--mode=read"]);
    assert!(!command_tokens_match_argv_pattern(
        &words(&["reader", "--mode=write", "source.rs"]),
        &pattern,
        false,
        "source.rs",
    ));
}
