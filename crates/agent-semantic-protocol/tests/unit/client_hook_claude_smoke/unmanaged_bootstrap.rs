use serde_json::json;

use super::{
    claude_fixture, codex_asp_query_payload, install_codex_hooks, prepend_path,
    run_codex_hook_decision_with_env, run_codex_pre_tool_decision_with_env,
};

#[test]
fn codex_bootstrap_registry_miss_requires_host_tree_audit_before_create() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000066";

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
        ])
        .current_dir(&root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("CODEX_HOME", &codex_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run bootstrap without registry or rollout candidate");
    let rendered = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{rendered}");
    assert!(rendered.contains("state: Audit"), "{rendered}");
    assert!(
        rendered.contains("why: registry-missing-host-tree-audit-required"),
        "{rendered}"
    );
    assert!(
        rendered.contains("1: audit-host-agent-tree-for-existing-resident-child"),
        "{rendered}"
    );
    assert!(
        rendered.contains("2: resume-existing-host-resident-child"),
        "{rendered}"
    );
    assert!(
        rendered.contains("3: audit-host-typed-spawn-schema"),
        "{rendered}"
    );
    assert!(
        rendered.contains("4: activate-inline-parser-fallback"),
        "{rendered}"
    );
    assert!(
        rendered.contains("5: create-managed-resident-child-after-host-tree-miss"),
        "{rendered}"
    );
    assert!(
        rendered.contains("requiredField=agent_type")
            && rendered.contains("genericFieldsInsufficient=task_name,message,fork_turns"),
        "{rendered}"
    );
    assert!(
        rendered.contains("inline-parser-fallback: available=true state=ReadyDegraded")
            && rendered.contains("optIn=ASP_INLINE_PARSER_FALLBACK=1")
            && rendered.contains("rawSourceFallback=false"),
        "{rendered}"
    );
    assert!(!rendered.contains("state: Create"), "{rendered}");
}

#[test]
fn codex_bootstrap_never_starts_codex_app_server() {
    use std::os::unix::fs::PermissionsExt;

    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000072";

    let start = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": "asp-bootstrap-performance-child",
            "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini",
            "reasoning_effort": "low",
            "permission_mode": "bypassPermissions",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(start["decision"].as_str(), Some("allow"));

    let fake_codex = root.join("blocking-codex");
    let marker = root.join("codex-app-server-invoked");
    std::fs::write(
        &fake_codex,
        "#!/bin/sh\nprintf invoked > \"$ASP_CODEX_PROBE_MARKER\"\nsleep 5\nexit 1\n",
    )
    .expect("write blocking fake Codex binary");
    let mut permissions = std::fs::metadata(&fake_codex)
        .expect("read fake Codex permissions")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&fake_codex, permissions).expect("make fake Codex binary executable");

    let started_at = std::time::Instant::now();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
            "--json",
        ])
        .current_dir(&root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("CODEX_HOME", &codex_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env("ASP_CODEX_BIN", &fake_codex)
        .env("ASP_CODEX_PROBE_MARKER", &marker)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run external-process-free bootstrap");
    let elapsed = started_at.elapsed();
    let rendered = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(output.status.success(), "{rendered}");
    assert!(!marker.exists(), "bootstrap invoked ASP_CODEX_BIN");
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "bootstrap blocked on external runtime discovery for {elapsed:?}: {rendered}"
    );
}

#[test]
fn codex_v2_default_agent_type_requires_typed_replacement_without_registration() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000067";
    let child_session_id = "019f126d-0000-7000-8000-000000000167";
    super::rollout_fixture::write_codex_v2_asp_explorer_rollout(
        &root,
        root_session_id,
        child_session_id,
    );

    let decision = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "default",
            "model": "gpt-5.4-mini",
            "reasoning_effort": "low",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("allow"));
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("replace-drifted-native-subagent")
    );
    assert_eq!(
        decision["fields"]["nextAction"].as_str(),
        Some("retire-drifted-child-and-create-configured-replacement")
    );
    assert_eq!(decision["fields"]["profileDriftDetected"], true);
}

#[test]
fn codex_native_asp_subagent_reasoning_mismatch_requests_typed_replacement() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000068";

    let decision = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": "019f126d-0000-7000-8000-000000000168",
            "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini",
            "reasoning_effort": "medium",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("allow"));
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("replace-drifted-native-subagent")
    );
    assert_eq!(
        decision["fields"]["agentSessionObservedReasoningEffort"].as_str(),
        Some("medium")
    );
    assert_eq!(
        decision["fields"]["expectedReasoningEffort"].as_str(),
        Some("low")
    );
    assert_eq!(
        decision["fields"]["nextAction"].as_str(),
        Some("retire-drifted-child-and-create-configured-replacement")
    );
}

#[test]
fn codex_bootstrap_requires_typed_replacement_for_inherited_subagent_model() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000064";

    let start = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": "default-native-child",
            "agent_type": "default",
            "model": "gpt-5.6-sol",
            "reasoning_effort": "medium",
            "permission_mode": "bypassPermissions",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(start["decision"].as_str(), Some("allow"));

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
        ])
        .current_dir(&root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("CODEX_HOME", &codex_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run bootstrap after unmanaged native child");
    let rendered = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{rendered}");
    assert!(rendered.contains("state: Repair"), "{rendered}");
    assert!(
        rendered.contains("why: typed-resident-replacement-required"),
        "{rendered}"
    );
    assert!(
        rendered.contains("expectedAgentType=asp_explorer observedAgentType=default"),
        "{rendered}"
    );
    assert!(rendered.contains("observedModel=gpt-5.6-sol"), "{rendered}");
    assert!(
        rendered.contains("expectedModel=gpt-5.4-mini observedModel=gpt-5.6-sol"),
        "{rendered}"
    );
    assert!(
        rendered.contains("expectedReasoning=low observedReasoning=medium"),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "action=retire-drifted-child-and-create-configured-replacement runtimeOverrideOwner=none runtimeSwitchIntentInFollowupMessage=false"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains("1: retire-drifted-child-and-create-configured-replacement"),
        "{rendered}"
    );
    assert!(rendered.contains("agent_type=asp_explorer"), "{rendered}");
    assert!(
        rendered.contains(
            "main-agent-control-directive: Retire/archive drifted target /root/asp_explorer and child default-native-child"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "host-control-contract: target=/root/asp_explorer identityPolicy=retire-before-replacement createPolicy=single-typed-replacement-only instructionMode=host-native-lifecycle"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "host-control-blocker: nextState=Blocked bootstrapBlocked=host-typed-resident-replacement-unavailable"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains("Codex must load the registered TOML"),
        "{rendered}"
    );
    assert!(
        rendered.contains("2: report-host-typed-replacement-unavailable"),
        "{rendered}"
    );
    assert!(
        rendered.contains("bootstrapBlocked=host-typed-resident-replacement-unavailable"),
        "{rendered}"
    );
    assert!(rendered.contains("after: Blocked"), "{rendered}");
    assert!(
        !rendered.contains("start-new-native-codex-task"),
        "{rendered}"
    );
}

#[test]
fn codex_bootstrap_json_emits_typed_replacement_directive() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000069";
    let child_session_id = "019f126d-0000-7000-8000-000000000169";

    let start = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "default",
            "model": "gpt-5.6-sol",
            "reasoning_effort": "xhigh",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(start["decision"].as_str(), Some("allow"));

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
            "--json",
        ])
        .current_dir(&root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("CODEX_HOME", &codex_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run JSON bootstrap after runtime drift");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rendered: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse bootstrap JSON");
    let directive = &rendered["hostControlDirective"];

    assert_eq!(rendered["state"], "Repair");
    assert_eq!(
        directive["intent"],
        "replace-drifted-resident-with-typed-role"
    );
    assert_eq!(directive["target"], "/root/asp_explorer");
    assert_eq!(directive["childSessionId"], child_session_id);
    assert_eq!(directive["identityPolicy"], "retire-before-replacement");
    assert_eq!(directive["createPolicy"], "single-typed-replacement-only");
    assert_eq!(directive["instructionMode"], "host-native-lifecycle");
    assert_eq!(directive["desiredRuntime"]["model"], "gpt-5.4-mini");
    assert_eq!(directive["desiredRuntime"]["reasoningEffort"], "low");
    assert_eq!(
        directive["controlChannel"]["requiredSurface"],
        "host-native-retire-and-typed-spawn"
    );
    assert_eq!(
        directive["controlChannel"]["requiredParameters"],
        json!(["target", "agent_type", "task_name", "fork_turns"])
    );
    assert_eq!(
        directive["controlChannel"]["runtimeApplication"],
        "codex-registered-role-config"
    );
    assert_eq!(
        directive["controlChannel"]["taskMessageCarriesControlIntent"],
        false
    );
    assert_eq!(
        directive["controlChannel"]["taskMessageIsRuntimeEvidence"],
        false
    );
    let expected_switch_message = "Retire the drifted resident and create one typed asp_explorer replacement from the registered Codex role. The expected runtime is model gpt-5.4-mini with reasoning low, but Codex must obtain both values from the role TOML rather than this message.";
    assert_eq!(
        directive["controlChannel"]["message"],
        expected_switch_message
    );
    assert_eq!(
        directive["mainAgentAction"]["surface"],
        "host-native-retire-and-typed-spawn"
    );
    assert_eq!(
        directive["mainAgentAction"]["arguments"]["target"],
        "/root/asp_explorer"
    );
    assert_eq!(
        directive["mainAgentAction"]["arguments"]["message"],
        expected_switch_message
    );
    assert_eq!(directive["unavailable"]["nextState"], "Blocked");
    assert_eq!(
        directive["unavailable"]["bootstrapBlocked"],
        "host-typed-resident-replacement-unavailable"
    );
    assert_eq!(rendered["choices"][1]["nextState"], "Blocked");
}

#[test]
fn codex_native_asp_subagent_model_mismatch_requests_typed_replacement() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000062";
    let child_session_id = "019f126d-0000-7000-8000-000000000162";

    let decision = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "asp_explorer",
            "model": "gpt-5.5",
            "reasoning_effort": "low",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("allow"));
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("replace-drifted-native-subagent")
    );
    assert_eq!(
        decision["fields"]["observedModel"].as_str(),
        Some("gpt-5.5")
    );
    assert_eq!(
        decision["fields"]["expectedModel"].as_str(),
        Some("gpt-5.4-mini")
    );
    assert_eq!(
        decision["fields"]["nextAction"].as_str(),
        Some("retire-drifted-child-and-create-configured-replacement")
    );
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("no same-child runtime override"));
    assert!(message.contains("agent_type=asp_explorer"));
    assert!(message.contains("retire/archive"));
    assert!(message.contains("must not block unrelated Codex tools"));
    assert!(!message.contains("Stop this new native child"));
}

#[test]
fn codex_bootstrap_replaces_registered_child_after_profile_drift() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000065";
    let child_session_id = "019f126d-0000-7000-8000-000000000165";
    let initial_start = run_codex_hook_decision_with_env(
        &root,
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
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(initial_start["decision"].as_str(), Some("allow"));

    let drifted_start = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "default",
            "model": "gpt-5.6-sol",
            "reasoning_effort": "medium",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(drifted_start["decision"].as_str(), Some("allow"));
    assert_eq!(
        drifted_start["fields"]["agentSessionAction"].as_str(),
        Some("replace-drifted-native-subagent")
    );
    let state_home = root.join(".agent-semantic-protocols");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
        ])
        .current_dir(&root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("CODEX_HOME", &codex_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_STATE_HOME", &state_home)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run bootstrap for registered model drift");
    let rendered = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{rendered}");
    assert!(rendered.contains("state: Repair"), "{rendered}");
    assert!(
        rendered.contains("why: typed-resident-replacement-required"),
        "{rendered}"
    );
    assert!(
        rendered.contains("model: expected gpt-5.4-mini"),
        "{rendered}"
    );
    assert!(
        rendered.contains("expectedModel=gpt-5.4-mini observedModel=gpt-5.6-sol"),
        "{rendered}"
    );
    assert!(
        rendered.contains("1: retire-drifted-child-and-create-configured-replacement"),
        "{rendered}"
    );
    assert!(
        rendered.contains("create exactly one replacement with agent_type=asp_explorer"),
        "{rendered}"
    );
    assert!(
        rendered.contains("2: report-host-typed-replacement-unavailable"),
        "{rendered}"
    );

    let stopped = run_codex_hook_decision_with_env(
        &root,
        "subagent-stop",
        json!({
            "hook_event_name": "SubagentStop",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "default",
            "model": "gpt-5.6-sol",
            "reasoning_effort": "medium",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(stopped["decision"].as_str(), Some("allow"));

    let replacement_child_id = "019f126d-0000-7000-8000-000000000265";
    let replacement_start = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": replacement_child_id,
            "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini",
            "reasoning_effort": "low",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(replacement_start["decision"].as_str(), Some("allow"));

    let repaired = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
        ])
        .current_dir(&root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("CODEX_HOME", &codex_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_STATE_HOME", &state_home)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run bootstrap after typed replacement");
    let repaired_rendered = format!(
        "{}{}",
        String::from_utf8_lossy(&repaired.stdout),
        String::from_utf8_lossy(&repaired.stderr)
    );
    assert!(repaired.status.success(), "{repaired_rendered}");
    assert!(
        repaired_rendered.contains("state: Ready"),
        "{repaired_rendered}"
    );
    assert!(
        repaired_rendered.contains("why: typed-resident-replacement-verified"),
        "{repaired_rendered}"
    );
    assert!(
        repaired_rendered.contains(replacement_child_id),
        "{repaired_rendered}"
    );
    assert!(
        !repaired_rendered.contains("retire-drifted-child-and-create-configured-replacement"),
        "{repaired_rendered}"
    );
}

#[test]
fn codex_completed_v2_child_keeps_typed_replacement_actionable() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000070";
    let child_session_id = "019f126d-0000-7000-8000-000000000170";

    let start = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "default",
            "model": "gpt-5.6-sol",
            "reasoning_effort": "xhigh",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(
        start["fields"]["agentSessionAction"],
        "replace-drifted-native-subagent"
    );

    let stop = run_codex_hook_decision_with_env(
        &root,
        "subagent-stop",
        json!({
            "hook_event_name": "SubagentStop",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "default",
            "model": "gpt-5.6-sol",
            "reasoning_effort": "xhigh",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(stop["decision"], "allow");
    assert_ne!(
        stop["fields"]["agentSessionAction"],
        "subagent-stop-archived-managed-child"
    );

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
        ])
        .current_dir(&root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("CODEX_HOME", &codex_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run bootstrap after completed drifted v2 child");
    let rendered = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{rendered}");
    assert!(rendered.contains("state: Repair"), "{rendered}");
    assert!(
        rendered.contains(&format!(
            "main-agent-control-directive: Retire/archive drifted target /root/asp_explorer and child {child_session_id}"
        )),
        "{rendered}"
    );
    assert!(!rendered.contains("state: Create"), "{rendered}");
}

#[test]
fn codex_repeated_drift_keeps_single_typed_replacement_action() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000073";
    let child_session_id = "019f126d-0000-7000-8000-000000000173";
    let start = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "default",
            "model": "gpt-5.6-sol",
            "reasoning_effort": "xhigh",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(
        start["fields"]["agentSessionAction"],
        "replace-drifted-native-subagent"
    );

    for index in 0..2 {
        let observed_model = if index == 0 {
            "gpt-5.6-sol"
        } else {
            "gpt-5.4-mini"
        };
        let stop = run_codex_hook_decision_with_env(
            &root,
            "subagent-stop",
            json!({
                "hook_event_name": "SubagentStop",
                "session_id": root_session_id,
                "agent_id": child_session_id,
                "agent_type": "default",
                "model": observed_model,
                "reasoning_effort": "xhigh",
                "permission_mode": "default",
            }),
            &[("CODEX_THREAD_ID", root_session_id)],
        );
        assert_eq!(stop["decision"], "allow");
        assert_eq!(stop["fields"]["hookObservedModel"], observed_model);
        assert_eq!(stop["fields"]["hookObservedReasoningEffort"], "xhigh");
    }

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
            "--json",
        ])
        .current_dir(&root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("CODEX_HOME", &codex_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run JSON bootstrap after repeated same-child drift");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rendered: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse bootstrap JSON");

    assert_eq!(rendered["state"], "Repair");
    assert_eq!(
        rendered["trace"][1]["result"],
        "typed-resident-replacement-required"
    );
    assert_eq!(
        rendered["hostLifecycleObservation"]["consecutiveObservationCount"],
        2
    );
    assert_eq!(
        rendered["hostControlDirective"]["intent"],
        "replace-drifted-resident-with-typed-role"
    );
    assert_eq!(
        rendered["hostControlDirective"]["unavailable"]["observedAfterSameChildResume"],
        false
    );
    assert_eq!(
        rendered["hostControlDirective"]["unavailable"]["bootstrapBlocked"],
        "host-typed-resident-replacement-unavailable"
    );
    assert!(!rendered["hostControlDirective"]["mainAgentAction"].is_null());
    assert!(!rendered["hostControlDirective"]["controlChannel"]["message"].is_null());
    assert_eq!(
        rendered["hostLifecycleObservation"]["driftDimensions"],
        json!(["reasoningEffort"])
    );
    assert_eq!(
        rendered["hostLifecycleObservation"]["repairAttemptStatus"],
        "typed-resident-replacement-required"
    );
    assert_eq!(rendered["choices"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        rendered["choices"][0]["id"],
        "retire-drifted-child-and-create-configured-replacement"
    );
}

#[test]
fn drifted_resident_route_never_blocks_unrelated_codex_tool_use() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000074";

    let start = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": "019f126d-0000-7000-8000-000000000174",
            "agent_type": "default",
            "model": "gpt-5.6-sol",
            "reasoning_effort": "xhigh",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(
        start["fields"]["agentSessionAction"],
        "replace-drifted-native-subagent"
    );

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("rg -n lifecycle crates"),
        &[("CODEX_THREAD_ID", root_session_id)],
    );

    assert_eq!(decision["decision"], "allow", "{decision}");
    assert_eq!(decision["reasonKind"], "none");
    assert_eq!(
        decision["fields"]["residentRoutePolicy"],
        "soft-nonblocking"
    );
    assert_eq!(
        decision["fields"]["residentRouteStatus"],
        "degraded-profile-or-runtime-drift"
    );
    for field in [
        "forbiddenUntilResolved",
        "requiredAction",
        "nextAction",
        "agentSessionLoopCommand",
        "agentSessionBootstrap",
    ] {
        assert!(
            decision["fields"].get(field).is_none(),
            "{field}: {decision}"
        );
    }
}

#[test]
fn repeated_bootstrap_locks_historical_identity_and_never_binds_from_path_presence() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000070";
    let child_a = "019f126d-0000-7000-8000-000000000170";
    let child_b = "019f126d-0000-7000-8000-000000000171";
    super::write_codex_asp_explore_rollout(&root, root_session_id, child_a, "gpt-5.4-mini");
    super::write_codex_asp_explore_rollout(&root, root_session_id, child_b, "gpt-5.4-mini");

    let run_bootstrap = || {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
            .args([
                "agent",
                "session",
                "bootstrap",
                "--name",
                "asp-explore",
                "--root-session-id",
                root_session_id,
                "--json",
            ])
            .current_dir(&root)
            .env("PATH", prepend_path(&root.join(".bin")))
            .env("CODEX_HOME", &codex_home)
            .env("CODEX_THREAD_ID", root_session_id)
            .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
            .env_remove("PRJ_CACHE_HOME")
            .output()
            .expect("run bootstrap for historical identity lock");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&output.stdout)
            .expect("parse historical identity bootstrap")
    };

    let first = run_bootstrap();
    let locked_child = first["session"]["childSessionId"]
        .as_str()
        .expect("first bootstrap selects one historical child")
        .to_string();
    assert!(
        locked_child == child_a || locked_child == child_b,
        "{first:#}"
    );
    assert_eq!(first["state"], "Audit", "{first:#}");
    assert_eq!(
        first["choices"][0]["id"],
        "audit-host-agent-tree-before-live-target-rebind"
    );

    let observation = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "observe-host-tree",
            "--name",
            "asp-explore",
            "--resident-target-status",
            "present",
        ])
        .current_dir(&root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("CODEX_HOME", &codex_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("record path-only host observation");
    assert!(observation.status.success());

    let second = run_bootstrap();
    assert_eq!(
        second["session"]["childSessionId"], locked_child,
        "repair bootstrap rotated to a different historical child: {second:#}"
    );
    assert_eq!(second["state"], "RebindExistingChildTarget", "{second:#}");
    assert_eq!(second["session"]["messageTargetStatus"], "unbound");
    assert_eq!(
        second["hostResidentTargetObservation"]["identityStatus"],
        "unverified"
    );

    let absent = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "observe-host-tree",
            "--name",
            "asp-explore",
            "--resident-target-status",
            "absent",
        ])
        .current_dir(&root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("CODEX_HOME", &codex_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("record target-not-found as host absence");
    assert!(absent.status.success());
    let typed = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "observe-host-capability",
            "--name",
            "asp-explore",
            "--agent-type-field",
            "present",
        ])
        .current_dir(&root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("CODEX_HOME", &codex_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("record typed spawn capability");
    assert!(typed.status.success());

    let third = run_bootstrap();
    assert_eq!(third["session"]["childSessionId"], locked_child);
    assert_eq!(third["state"], "Audit", "{third:#}");
    assert_eq!(third["bootstrapBlocked"], serde_json::Value::Null);
    assert_eq!(
        third["canonicalBindingObservation"]["status"],
        "historical-only-non-rebindable"
    );
    assert_eq!(
        third["canonicalBindingObservation"]["reasoningGateEvaluated"],
        false
    );
    assert_eq!(
        third["canonicalBindingObservation"]["nextAction"],
        "create-canonical-typed-child-after-orphaned-owner"
    );
    assert!(third.get("hostLifecycleObservation").is_none(), "{third:#}");
    assert_eq!(
        third["choices"][0]["id"], "create-canonical-typed-child-after-orphaned-owner",
        "{third:#}"
    );
    assert!(
        third["choices"]
            .as_array()
            .expect("choices")
            .iter()
            .all(|choice| choice["id"] != "resume-existing-child-for-live-target-rebind")
    );
}

#[test]
fn typed_subagent_start_supersedes_pre_spawn_target_absence() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000072";
    let child_session_id = "019f126d-0000-7000-8000-000000000172";
    let asp_state_home = root.join(".agent-semantic-protocols");

    let run_session = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
            .args(args)
            .current_dir(&root)
            .env("PATH", prepend_path(&root.join(".bin")))
            .env("CODEX_HOME", &codex_home)
            .env("CODEX_THREAD_ID", root_session_id)
            .env("ASP_STATE_HOME", &asp_state_home)
            .env_remove("PRJ_CACHE_HOME")
            .output()
            .expect("run agent session command")
    };
    let absent = run_session(&[
        "agent",
        "session",
        "observe-host-tree",
        "--name",
        "asp-explore",
        "--resident-target-status",
        "absent",
    ]);
    assert!(absent.status.success());

    let decision = super::run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", "")],
    );
    assert_eq!(decision["decision"], "allow");

    let bootstrap = run_session(&[
        "agent",
        "session",
        "bootstrap",
        "--name",
        "asp-explore",
        "--root-session-id",
        root_session_id,
        "--json",
    ]);
    assert!(bootstrap.status.success());
    let receipt: serde_json::Value =
        serde_json::from_slice(&bootstrap.stdout).expect("parse bootstrap receipt");
    assert_eq!(
        receipt["hostResidentTargetObservation"]["targetStatus"], "present",
        "{receipt:#}"
    );
    assert_eq!(
        receipt["hostResidentTargetObservation"]["identityStatus"],
        "verified"
    );
    assert_eq!(
        receipt["hostResidentTargetObservation"]["source"],
        "codex.subagent-start"
    );
    assert!(
        receipt["choices"]
            .as_array()
            .expect("choices")
            .iter()
            .all(|choice| !choice["id"].as_str().unwrap_or_default().contains("create")),
        "{receipt:#}"
    );
}
