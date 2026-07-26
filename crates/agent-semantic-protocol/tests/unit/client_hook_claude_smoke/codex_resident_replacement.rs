use super::{
    claude_fixture, install_codex_hooks, register_asp_explore_session,
    run_codex_hook_decision_with_env, show_agent_session_json,
};
use serde_json::json;
use std::path::Path;

#[test]
fn codex_subagent_start_requires_canonical_probe_before_replacement() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000035";
    let stale_child_id = "019f126d-0000-7000-8000-000000000135";
    let blocked_child_id = "019f126d-0000-7000-8000-000000000235";
    let replacement_child_id = "019f126d-0000-7000-8000-000000000335";
    register_asp_explore_session(&root, root_session_id, stale_child_id);

    let observation = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("CODEX_THREAD_ID", root_session_id)
        .env("CODEX_HOME", root.join(".codex-home"))
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .args([
            "agent",
            "session",
            "observe-host-tree",
            "--name",
            "asp-explore",
            "--resident-target-status",
            "absent",
        ])
        .output()
        .expect("record absent host-tree observation");
    assert!(
        observation.status.success(),
        "observe-host-tree failed: {}",
        String::from_utf8_lossy(&observation.stderr)
    );

    let blocked = native_resident_start(&root, root_session_id, blocked_child_id);
    assert_eq!(blocked["decision"].as_str(), Some("deny"), "{blocked}");
    assert_eq!(
        blocked["fields"]["agentSessionAction"].as_str(),
        Some("reuse-resident-child"),
        "{blocked}"
    );
    assert_eq!(
        blocked["fields"]["agentSessionDuplicateChildAction"].as_str(),
        Some("close-native-subagent"),
        "{blocked}"
    );
    assert_eq!(
        blocked["fields"]["childSessionId"].as_str(),
        Some(stale_child_id),
        "{blocked}"
    );
    assert_eq!(
        blocked["fields"]["agentSessionDuplicateChildId"].as_str(),
        Some(blocked_child_id),
        "{blocked}"
    );

    let canonical_probe_miss = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("CODEX_THREAD_ID", root_session_id)
        .env("CODEX_HOME", root.join(".codex-home"))
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .args([
            "agent",
            "session",
            "observe-host-tree",
            "--name",
            "asp-explore",
            "--resident-target-status",
            "unroutable",
            "--canonical-target",
            "/root/asp_explorer",
            "--evidence-ref",
            "canonical-followup-not-found:1",
        ])
        .output()
        .expect("record canonical probe miss");
    assert!(
        canonical_probe_miss.status.success(),
        "observe canonical probe miss failed: {}",
        String::from_utf8_lossy(&canonical_probe_miss.stderr)
    );

    let replacement = native_resident_start(&root, root_session_id, replacement_child_id);
    assert_eq!(
        replacement["decision"].as_str(),
        Some("allow"),
        "{replacement}"
    );
    let replacement_record = show_agent_session_json(&root, replacement_child_id);
    assert_eq!(
        replacement_record["sessions"][0]["sessionId"].as_str(),
        Some(replacement_child_id),
        "decision={replacement} record={replacement_record}"
    );

    let duplicate_child_id = "019f126d-0000-7000-8000-000000000435";
    let duplicate = native_resident_start(&root, root_session_id, duplicate_child_id);
    assert_eq!(duplicate["decision"].as_str(), Some("deny"), "{duplicate}");
    assert_eq!(
        duplicate["fields"]["childSessionId"].as_str(),
        Some(replacement_child_id),
        "{duplicate}"
    );
    assert_eq!(
        duplicate["fields"]["agentSessionDuplicateChildId"].as_str(),
        Some(duplicate_child_id),
        "{duplicate}"
    );
}

#[test]
fn codex_hidden_but_routable_child_resumes_before_replacement() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000036";
    let resident_child_id = "019f126d-0000-7000-8000-000000000136";
    let duplicate_child_id = "019f126d-0000-7000-8000-000000000236";

    let start = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": resident_child_id,
            "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(start["decision"].as_str(), Some("allow"), "{start}");

    let run_session_command = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&root)
            .env("CODEX_THREAD_ID", root_session_id)
            .env("CODEX_HOME", &codex_home)
            .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
            .env_remove("PRJ_CACHE_HOME")
            .args(["agent", "session"])
            .args(args)
            .output()
            .expect("run agent session command")
    };

    let absent = run_session_command(&[
        "observe-host-tree",
        "--name",
        "asp-explore",
        "--resident-target-status",
        "absent",
    ]);
    assert!(
        absent.status.success(),
        "observe absent failed: {}",
        String::from_utf8_lossy(&absent.stderr)
    );

    let before_probe = run_session_command(&[
        "bootstrap",
        "--name",
        "asp-explore",
        "--root-session-id",
        root_session_id,
        "--json",
    ]);
    assert!(before_probe.status.success());
    let before_probe: serde_json::Value =
        serde_json::from_slice(&before_probe.stdout).expect("pre-probe bootstrap JSON");
    assert_eq!(
        before_probe["state"].as_str(),
        Some("Audit"),
        "{before_probe}"
    );
    assert_eq!(
        before_probe["choices"][0]["id"].as_str(),
        Some("probe-hidden-routable-child-before-replacement"),
        "{before_probe}"
    );

    let ack = run_session_command(&[
        "observe-host-ack",
        "--name",
        "asp-explore",
        "--canonical-target",
        "/root/asp_explorer",
        "--evidence-ref",
        "canonical-followup-ok:1",
    ]);
    assert!(
        ack.status.success(),
        "observe canonical ack failed: {}",
        String::from_utf8_lossy(&ack.stderr)
    );

    let resumed = run_session_command(&[
        "bootstrap",
        "--name",
        "asp-explore",
        "--root-session-id",
        root_session_id,
        "--json",
    ]);
    assert!(
        resumed.status.success(),
        "bootstrap after canonical ack failed: {}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    let resumed: serde_json::Value =
        serde_json::from_slice(&resumed.stdout).expect("resumed bootstrap JSON");
    assert_eq!(resumed["state"].as_str(), Some("Ready"), "{resumed}");
    assert_eq!(
        resumed
            .pointer("/hostControlDirective/intent")
            .and_then(serde_json::Value::as_str),
        Some("same-child-followup-ack-rebind"),
        "{resumed}"
    );
    assert_eq!(
        resumed
            .pointer("/hostLifecycleObservation/childSessionId")
            .and_then(serde_json::Value::as_str),
        Some(resident_child_id),
        "{resumed}"
    );
    assert_eq!(
        resumed
            .pointer("/hostLifecycleObservation/sameChildIdentity")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "{resumed}"
    );

    let duplicate = native_resident_start(&root, root_session_id, duplicate_child_id);
    assert_eq!(duplicate["decision"].as_str(), Some("deny"), "{duplicate}");
    assert_eq!(
        duplicate["fields"]["agentSessionAction"].as_str(),
        Some("resume-existing-resident-child"),
        "{duplicate}"
    );
    assert_eq!(
        duplicate["fields"]["childSessionId"].as_str(),
        Some(resident_child_id),
        "{duplicate}"
    );
    assert_eq!(
        duplicate["fields"]["agentSessionDuplicateChildId"].as_str(),
        Some(duplicate_child_id),
        "{duplicate}"
    );
}

fn native_resident_start(
    root: &Path,
    root_session_id: &str,
    child_session_id: &str,
) -> serde_json::Value {
    run_codex_hook_decision_with_env(
        root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini",
            "reasoning_effort": "low",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", "")],
    )
}
