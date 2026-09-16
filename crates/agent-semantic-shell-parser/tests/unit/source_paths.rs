// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_shell_parser::embedded_literal_candidates;

#[test]
fn git_object_path_is_an_embedded_literal_candidate() {
    assert_eq!(
        embedded_literal_candidates(&[
            "show".to_string(),
            "HEAD:crates/example/src/lib.rs".to_string(),
        ]),
        ["crates/example/src/lib.rs"]
    );
}

#[test]
fn revision_qualified_source_is_projected_once_by_a_command_stage() {
    let stages = agent_semantic_shell_parser::parse_bash_command_candidates(
        "git show HEAD:crates/example/src/lib.rs",
    )
    .expect("parse revision-qualified source");
    let projected = agent_semantic_shell_parser::command_stage_source_paths(&stages[0]);
    assert_eq!(
        projected
            .iter()
            .filter(|candidate| candidate.as_str() == "crates/example/src/lib.rs")
            .count(),
        1
    );
    assert!(
        !projected
            .iter()
            .any(|candidate| candidate.starts_with("HEAD:"))
    );
}

#[test]
fn url_and_windows_drive_tokens_are_not_git_object_paths() {
    assert!(
        embedded_literal_candidates(&[
            "https://example.test/source.rs".to_string(),
            r"C:\workspace\source.rs".to_string(),
        ])
        .is_empty()
    );
}

#[test]
fn direct_read_is_a_bounded_command_stage() {
    let stages = agent_semantic_shell_parser::parse_bash_command_candidates(
        "read crates/agent-semantic-hook/src/lib.rs",
    )
    .expect("parse direct read");
    assert_eq!(
        stages.iter().map(|stage| stage.words()).collect::<Vec<_>>(),
        [as_strings(&[
            "read",
            "crates/agent-semantic-hook/src/lib.rs",
        ])]
    );
    assert_eq!(
        agent_semantic_shell_parser::command_stages_match_wrapped_prefix(
            &stages,
            &as_strings(&["read"]),
        ),
        agent_semantic_shell_parser::PrefixMatch::Matched
    );
}

fn as_strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}
