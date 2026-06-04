use agent_semantic_hook::source_access::SourceAccessDecisionKind;

use super::source_access;
use super::support::{args, temp_root, write_activation};

#[test]
fn read_file_command_returns_hard_deny_for_source_path() {
    let root = temp_root();
    let activation = write_activation(&root, "typescript");
    let decision = source_access::source_access_decision_from_args(&args(&[
        "read-file",
        "--activation",
        activation.to_str().expect("path"),
        "--rpc-method",
        "fs/readFile",
        "src/cli/agent-hooks.ts",
    ]))
    .expect("decision")
    .expect("source decision");
    let value = serde_json::to_value(&decision).expect("json");
    assert_eq!(decision.decision, SourceAccessDecisionKind::Deny);
    assert_eq!(value["providerId"], "ts-harness");
    assert_eq!(
        decision.routes[0].argv,
        args(&[
            "asp",
            "typescript",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/cli/agent-hooks.ts",
            "--code",
            ".",
        ])
    );
}

#[test]
fn shell_egress_command_returns_suppress_for_source_path() {
    let root = temp_root();
    let activation = write_activation(&root, "typescript");
    let decision = source_access::source_access_decision_from_args(&args(&[
        "shell-egress",
        "--activation",
        activation.to_str().expect("path"),
        "--command",
        "sed -n '1,120p' src/cli/agent-hooks.ts",
        "--output-digest",
        "sha256:source-like-output",
        "src/cli/agent-hooks.ts",
    ]))
    .expect("decision")
    .expect("source decision");
    let value = serde_json::to_value(&decision).expect("json");
    assert_eq!(decision.decision, SourceAccessDecisionKind::Suppress);
    assert!(decision.source_bytes_returned);
    assert!(!decision.model_visible_bytes_returned);
    assert_eq!(value["providerId"], "ts-harness");
}

#[test]
fn source_access_command_returns_none_for_non_source_path() {
    let root = temp_root();
    let activation = write_activation(&root, "typescript");
    let decision = source_access::source_access_decision_from_args(&args(&[
        "read-file",
        "--activation",
        activation.to_str().expect("path"),
        "README.md",
    ]))
    .expect("decision");
    assert!(decision.is_none());
}
