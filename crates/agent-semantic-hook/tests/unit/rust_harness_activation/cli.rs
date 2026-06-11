use std::io::Write;
use std::process::Stdio;

mod activation_sync;
mod hook;
mod install;

use super::support::{
    asp_command, temp_project_root, write_default_client_hook_config,
    write_root_owned_rust_activation,
};

#[test]
fn cli_doctor_accepts_root_owned_rust_activation() {
    let root = temp_project_root("doctor-activation");
    let activation_path = write_root_owned_rust_activation(&root);
    let output = asp_command()
        .args([
            "hook",
            "doctor",
            "--activation",
            activation_path.to_str().expect("utf8 activation path"),
            root.to_str().expect("utf8 project root"),
        ])
        .output()
        .expect("run agent-semantic-protocol doctor");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("doctor stdout");
    assert!(stdout.contains("[agent-doctor] status=ok"));
    assert!(stdout.contains("providers=1"));
    assert!(stdout.contains("clientConfigStatus=missing"));
    assert!(stdout.contains("classifierProbe=unavailable"));
    assert!(stdout.contains("classifierReason=client-config-missing"));
    assert!(stdout.contains("enforcement=unavailable"));
    assert!(stdout.contains("enforcementProbe=skipped"));
    assert!(stdout.contains("enforcementReason=project-hook-missing"));
    assert!(stdout.contains("|provider language=rust provider=rs-harness"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_doctor_reports_classifier_probe_for_codex_exec_command_source_dump() {
    let root = temp_project_root("doctor-classifier-probe");
    let activation_path = write_root_owned_rust_activation(&root);
    write_default_client_hook_config(&root);
    let output = asp_command()
        .args([
            "hook",
            "doctor",
            "--activation",
            activation_path.to_str().expect("utf8 activation path"),
            root.to_str().expect("utf8 project root"),
        ])
        .output()
        .expect("run agent-semantic-protocol doctor");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("doctor stdout");
    assert!(stdout.contains("[agent-doctor] status=ok"));
    assert!(stdout.contains("clientConfigStatus=ok"));
    assert!(stdout.contains("classifierProbe=deny"));
    assert!(stdout.contains("classifierReason=bulk-source-dump"));
    assert!(stdout.contains("enforcement=unavailable"));
    assert!(stdout.contains("enforcementProbe=skipped"));
    assert!(stdout.contains("enforcementReason=project-hook-missing"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_emits_decision_for_root_owned_rust_activation() {
    let root = temp_project_root("hook-activation");
    let activation_path = write_root_owned_rust_activation(&root);
    let mut child = asp_command()
        .args([
            "hook",
            "--client",
            "codex",
            "pre-tool",
            "--activation",
            activation_path.to_str().expect("utf8 activation path"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run asp hook");
    child
        .stdin
        .as_mut()
        .expect("hook stdin")
        .write_all(br#"{"tool_name":"Read","tool_input":{"path":"src/lib.rs"}}"#)
        .expect("write hook payload");

    let output = child.wait_with_output().expect("wait for hook output");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("hook JSON");
    assert_eq!(value["hookSpecificOutput"]["permissionDecision"], "deny");
    let context = value["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("decision context");
    assert!(context.starts_with("[agent-hook-decision] "));
    assert!(context.contains("\"decision\":\"deny\""));
    assert!(context.contains("\"reasonKind\":\"direct-source-read\""));
    let reason = value["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("permission reason");
    assert!(reason.contains("# ASP Hook Recovery"));
    assert!(reason.contains("blocked `direct-source-read`"));
    let system_message = value["systemMessage"].as_str().expect("system message");
    assert!(system_message.contains("asp rust query --selector src/lib.rs --workspace . --code"));
    assert!(system_message.contains("Do not retry `Read`, `cat`, `sed`, `rg`"));
    assert!(context.contains("\"binary\":\"asp\""));
    assert!(context.contains("\"src/lib.rs\""));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_denies_codex_exec_command_source_dump() {
    let root = temp_project_root("hook-exec-command-source-dump");
    let activation_path = write_root_owned_rust_activation(&root);
    let mut child = asp_command()
        .args([
            "hook",
            "--client",
            "codex",
            "pre-tool",
            "--activation",
            activation_path.to_str().expect("utf8 activation path"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run asp hook");
    child
        .stdin
        .as_mut()
        .expect("hook stdin")
        .write_all(
            br#"{"tool_name":"functions.exec_command","tool_input":{"cmd":"nl -ba src/lib.rs"}}"#,
        )
        .expect("write hook payload");

    let output = child.wait_with_output().expect("wait for hook output");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("hook JSON");
    assert_eq!(value["hookSpecificOutput"]["permissionDecision"], "deny");
    let context = value["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("decision context");
    assert!(context.contains("\"reasonKind\":\"bulk-source-dump\""));
    assert!(context.contains("\"toolName\":\"functions.exec_command\""));
    assert!(context.contains("\"src/lib.rs\""));
    let system_message = value["systemMessage"].as_str().expect("system message");
    assert!(system_message.contains("Do not retry `Read`, `cat`, `sed`, `rg`"));
    assert!(system_message.contains("asp rust query --selector src/lib.rs --workspace . --code"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_discovers_parent_activation_from_child_workdir() {
    let root = temp_project_root("hook-parent-activation");
    let activation_path = write_root_owned_rust_activation(&root);
    let default_activation_path = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("hooks")
        .join("activation.json");
    std::fs::create_dir_all(default_activation_path.parent().expect("activation parent"))
        .expect("create activation directory");
    std::fs::copy(&activation_path, &default_activation_path).expect("copy activation");
    let child_dir = root.join("nested").join("agent");
    std::fs::create_dir_all(&child_dir).expect("create child workdir");

    let mut child = asp_command()
        .current_dir(&child_dir)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["hook", "--client", "codex", "pre-tool"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run asp hook");
    child
        .stdin
        .as_mut()
        .expect("hook stdin")
        .write_all(br#"{"tool_name":"Read","tool_input":{"path":"src/lib.rs"}}"#)
        .expect("write hook payload");

    let output = child.wait_with_output().expect("wait for hook output");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("hook JSON");
    assert_eq!(value["hookSpecificOutput"]["permissionDecision"], "deny");
    let context = value["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("decision context");
    assert!(context.contains("\"reasonKind\":\"direct-source-read\""));
    assert!(context.contains("\"src/lib.rs\""));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_can_emit_raw_decision_for_schema_tests() {
    let root = temp_project_root("hook-decision-activation");
    let activation_path = write_root_owned_rust_activation(&root);
    let mut child = asp_command()
        .args([
            "hook",
            "--client",
            "codex",
            "pre-tool",
            "--activation",
            activation_path.to_str().expect("utf8 activation path"),
            "--emit",
            "decision",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run asp hook");
    child
        .stdin
        .as_mut()
        .expect("hook stdin")
        .write_all(br#"{"tool_name":"Read","tool_input":{"path":"src/lib.rs"}}"#)
        .expect("write hook payload");

    let output = child.wait_with_output().expect("wait for hook output");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("hook JSON");
    assert_eq!(value["decision"], "deny");
    assert_eq!(value["reasonKind"], "direct-source-read");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_blocks_subagent_stop_without_search_receipt() {
    let root = temp_project_root("subagent-stop-activation");
    let activation_path = write_root_owned_rust_activation(&root);
    let mut child = asp_command()
        .args([
            "hook",
            "--client",
            "codex",
            "subagent-stop",
            "--activation",
            activation_path.to_str().expect("utf8 activation path"),
            "--emit",
            "decision",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run asp hook");
    child
        .stdin
        .as_mut()
        .expect("hook stdin")
        .write_all(br#"{"hook_event_name":"SubagentStop","last_assistant_message":"done"}"#)
        .expect("write hook payload");

    let output = child.wait_with_output().expect("wait for hook output");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("hook JSON");
    assert_eq!(value["decision"], "block");
    assert_eq!(value["reasonKind"], "subagent-receipt-required");
    assert!(
        value["message"]
            .as_str()
            .expect("message")
            .contains("[search-subagent]")
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}
