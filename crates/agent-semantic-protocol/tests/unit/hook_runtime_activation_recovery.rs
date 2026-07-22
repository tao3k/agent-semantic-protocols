use std::path::Path;

use super::{ReasonKind, activation_load_failure_decision, is_activation_recovery_command};

fn bash_decision(command: &str) -> Option<agent_semantic_hook::HookDecision> {
    activation_load_failure_decision(
        "codex",
        "pre-tool",
        Path::new("/missing/activation.json"),
        "missing",
        &serde_json::json!({
            "tool_name": "Bash",
            "tool_input": { "cmd": command }
        })
        .to_string(),
    )
}

#[test]
fn activation_repair_commands_remain_available() {
    for command in ["asp hook doctor", "env HOME=/tmp asp hook doctor"] {
        assert!(is_activation_recovery_command(command), "{command}");
        assert!(bash_decision(command).is_none(), "{command}");
    }
}

#[test]
fn activation_failure_uses_shared_command_intent() {
    for command in [
        "cargo test -p agent-semantic-protocol",
        "mv src/old.rs src/new.rs",
        "touch .cache/activation-repair",
        "git diff -- src/lib.rs",
    ] {
        let decision = bash_decision(command).expect("non-recovery command must fail closed");
        assert_eq!(
            decision.reason_kind,
            ReasonKind::ActivationUnavailable,
            "{command}"
        );
    }

    for command in [
        "asp sync",
        "direnv exec . asp sync",
        "direnv exec . cat src/lib.rs",
        "nl -ba src/lib.rs | sed -n '1,40p'",
    ] {
        let decision = bash_decision(command).expect("non-doctor command must fail closed");
        assert_eq!(
            decision.reason_kind,
            ReasonKind::ActivationUnavailable,
            "{command}"
        );
    }
}

#[test]
fn structured_read_tools_still_fail_closed() {
    let decision = activation_load_failure_decision(
        "codex",
        "pre-tool",
        Path::new("/missing/activation.json"),
        "missing",
        &serde_json::json!({
            "tool_name": "Read",
            "tool_input": { "file_path": "src/lib.rs" }
        })
        .to_string(),
    )
    .expect("structured read decision");
    assert_eq!(decision.reason_kind, ReasonKind::ActivationUnavailable);
}
