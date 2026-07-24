use agent_semantic_hook::source_access::SourceAccessDecisionKind;

use super::source_access;
use super::support::{args, temp_root, write_activation};

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
fn source_access_help_exits_success() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["source-access", "--help"])
        .output()
        .expect("run source-access help");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help stdout");
    assert!(stdout.contains("source-access shell-egress"));
    assert!(!stdout.contains("read-file"));
}
