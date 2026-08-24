use agent_semantic_hook::canonical_recovery_admission;

fn payload(command: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "tool_input": { "command": command }
    }))
    .expect("serialize Hook payload")
}

#[test]
fn admits_only_recovery_hook_events() {
    let input = payload("asp server status");
    assert!(canonical_recovery_admission(Some("pre-tool"), &input));
    assert!(canonical_recovery_admission(
        Some("permission-request"),
        &input
    ));
    assert!(!canonical_recovery_admission(Some("post-tool"), &input));
}

#[test]
fn admits_exact_server_and_doctor_commands() {
    for command in [
        "asp server status",
        "asp server start",
        "direnv exec . asp server restart",
        "asp hook doctor --client codex",
    ] {
        assert!(canonical_recovery_admission(
            Some("pre-tool"),
            &payload(command)
        ));
    }
}

#[test]
fn admits_only_canonical_binary_install_for_matcher_recovery() {
    for command in ["/tmp/candidate/asp install binary"] {
        assert!(canonical_recovery_admission(
            Some("pre-tool"),
            &payload(command)
        ));
    }
}

#[test]
fn rejects_chains_untrusted_prefixes_and_extra_arguments() {
    for command in [
        "asp server status --json",
        "sudo asp server status",
        "asp server status; touch denied",
        "asp hook doctor --client codex --repair",
    ] {
        assert!(!canonical_recovery_admission(
            Some("pre-tool"),
            &payload(command)
        ));
    }
}
