use serde_json::json;

use super::{claude_fixture, install_codex_hooks, run_codex_pre_tool_decision_with_env};

#[test]
fn codex_normal_task_allows_precise_native_file_reads_for_any_source_extension() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);

    for (suffix, relative_path) in [
        ("rust", "src/core.rs"),
        ("hook", "src/hook_runtime.rs"),
        ("typescript", "src/component.ts"),
        ("tsx", "src/view.tsx"),
    ] {
        let path = root.join(relative_path);
        std::fs::write(&path, "// precise native read fixture\n").expect("write read fixture");
        let session_id = format!("019f5c0f-a653-7040-ab54-read-{suffix}");
        let decision = run_codex_pre_tool_decision_with_env(
            &root,
            json!({
                "session_id": session_id,
                "transcript_path": format!("/tmp/rollout-read-{suffix}.jsonl"),
                "cwd": root,
                "tool_name": "Read",
                "tool_input": {"file_path": path}
            }),
            &[],
        );

        assert_eq!(decision["decision"], "allow", "{relative_path}: {decision}");
        assert_eq!(
            decision["reasonKind"], "none",
            "{relative_path}: {decision}"
        );
        assert!(
            decision["fields"].get("agentSessionRoute").is_none(),
            "{relative_path}: {decision}"
        );
        assert!(
            decision["fields"].get("executionLane").is_none(),
            "{relative_path}: {decision}"
        );
    }
}

#[test]
fn codex_normal_task_routes_reasoning_from_payload_session_without_thread_env() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let session_id = "019f5c0f-a653-7040-ab54-514605a1c72a";
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": session_id,
            "transcript_path": format!("/tmp/rollout-{session_id}.jsonl"),
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": "asp rust guide"}
        }),
        &[],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["reasonKind"].as_str(),
        Some("asp-reasoning-routed")
    );
    assert_eq!(decision["fields"]["aspCommandIntent"], "reasoning");
    assert_eq!(decision["fields"]["aspCommandRoute"], "guide");
    assert_eq!(decision["fields"]["agentSessionRoute"], "asp-explore");
    assert_eq!(decision["fields"]["rootSessionId"], session_id);
    assert_eq!(decision["fields"]["sessionId"], session_id);
}

#[test]
fn codex_normal_task_routes_root_language_guide_spelling_as_reasoning() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let session_id = "019f5c0f-a653-7040-ab54-root-guide";
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": session_id,
            "transcript_path": "/tmp/rollout-root-guide.jsonl",
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": "asp guide --language rust"}
        }),
        &[],
    );

    assert_eq!(decision["decision"], "deny", "{decision}");
    assert_eq!(decision["reasonKind"], "asp-reasoning-routed", "{decision}");
    assert_eq!(decision["fields"]["aspCommandIntent"], "reasoning");
    assert_eq!(decision["fields"]["aspCommandRoute"], "guide");
    assert_eq!(decision["fields"]["agentSessionRoute"], "asp-explore");
}

#[test]
fn codex_normal_task_preserves_non_reasoning_intents_from_payload_session() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);

    for (suffix, command, expected_decision, expected_intent) in [
        (
            "invalid",
            "asp rust query --selector src/lib.rs --workspace . --code",
            "deny",
            "invalid-evidence",
        ),
        (
            "exact",
            "asp rust query --selector rust://src/lib.rs#item/function/run --workspace . --code",
            "allow",
            "exact-evidence",
        ),
        (
            "fallback",
            "asp rust query --from-hook direct-source-read --selector src/lib.rs:1:10 --workspace . --code --fallback-reason bounded",
            "allow",
            "direct-read-fallback",
        ),
    ] {
        let session_id = format!("019f5c0f-a653-7040-ab54-{suffix}");
        let decision = run_codex_pre_tool_decision_with_env(
            &root,
            json!({
                "session_id": session_id,
                "transcript_path": format!("/tmp/rollout-{suffix}.jsonl"),
                "cwd": root,
                "tool_name": "Bash",
                "tool_input": {"command": command}
            }),
            &[],
        );

        assert_eq!(decision["decision"], expected_decision, "{command}");
        assert_eq!(
            decision["fields"]["aspCommandIntent"], expected_intent,
            "{command}"
        );
        assert!(
            decision["fields"].get("agentSessionRoute").is_none(),
            "{command}: {decision}"
        );
    }
}

#[test]
fn codex_normal_task_does_not_route_configured_control_plane_command() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let session_id = "019f5c0f-a653-7040-ab54-control";
    let command = "asp install plugin --codex .";
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": session_id,
            "transcript_path": "/tmp/rollout-control.jsonl",
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": command}
        }),
        &[],
    );

    assert_eq!(decision["decision"], "allow", "{decision}");
    assert!(
        decision["fields"].get("agentSessionRoute").is_none(),
        "{decision}"
    );
    assert!(
        decision["fields"].get("aspCommandIntent").is_none(),
        "{decision}"
    );
}

#[test]
fn codex_normal_task_matches_only_supported_asp_invocations() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);

    for (suffix, command, expected_decision, expected_reason) in [
        (
            "semicolon",
            "printf ready; asp rust guide",
            "deny",
            "asp-reasoning-routed",
        ),
        ("data", "printf '%s' 'asp rust guide'", "allow", "none"),
        ("invalid-facade", "asp bananas guide", "deny", "none"),
    ] {
        let session_id = format!("019f5c0f-a653-7040-ab54-{suffix}");
        let decision = run_codex_pre_tool_decision_with_env(
            &root,
            json!({
                "session_id": session_id,
                "transcript_path": format!("/tmp/rollout-{suffix}.jsonl"),
                "cwd": root,
                "tool_name": "Bash",
                "tool_input": {"command": command}
            }),
            &[],
        );

        assert_eq!(decision["decision"], expected_decision, "{decision}");
        assert_eq!(decision["reasonKind"], expected_reason, "{decision}");
        if suffix == "invalid-facade" {
            assert_eq!(decision["fields"]["invalidFacade"], "bananas", "{decision}");
            assert!(
                decision["fields"].get("agentSessionRoute").is_none(),
                "{decision}"
            );
        }
    }
}

#[test]
fn codex_normal_task_does_not_treat_asp_path_inspection_as_an_invocation() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let session_id = "019f5c0f-a653-7040-ab54-path-inspection";
    let command =
        "command -v asp; readlink \"$(command -v asp)\"; shasum -a 256 \"$(command -v asp)\"";
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": session_id,
            "transcript_path": "/tmp/rollout-path-inspection.jsonl",
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": command}
        }),
        &[],
    );

    assert_eq!(decision["decision"], "allow", "{decision}");
    assert_eq!(decision["reasonKind"], "none", "{decision}");
    assert!(
        decision["fields"].get("aspCommandIntent").is_none(),
        "{decision}"
    );
    assert!(
        decision["fields"].get("agentSessionRoute").is_none(),
        "{decision}"
    );
}

#[test]
fn codex_normal_task_allows_testing_execution_with_digest_receipt() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let session_id = "019f5c0f-a653-7040-ab54-testing";
    let command = "cargo test -p agent-semantic-config";
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": session_id,
            "transcript_path": "/tmp/rollout-testing.jsonl",
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": command}
        }),
        &[],
    );

    assert_eq!(decision["decision"], "allow", "{decision}");
    assert_eq!(decision["reasonKind"], "none", "{decision}");
    assert_eq!(decision["fields"]["executionLane"], "asp-testing");
    assert_eq!(decision["fields"]["executionTransport"], "current-session");
    assert_eq!(
        decision["fields"]["executionReceiptKind"],
        "asp-testing-execution-v1"
    );
    assert_eq!(decision["fields"]["rootSessionId"], session_id);
    assert!(
        decision["fields"]["executionCommandDigest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:") && digest.len() == 71),
        "{decision}"
    );
    assert!(
        decision["fields"].get("agentSessionRoute").is_none(),
        "{decision}"
    );
}
