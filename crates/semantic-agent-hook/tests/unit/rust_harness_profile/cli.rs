use std::io::Write;
use std::process::{Command, Stdio};

mod hook;
mod install;
mod install_julia;
mod profile_sync;
mod profiles;

use super::support::{temp_project_root, write_root_owned_rust_profile_registry};

#[test]
fn cli_doctor_accepts_root_owned_rust_profile_registry() {
    let root = temp_project_root("doctor-profile");
    let profile_path = write_root_owned_rust_profile_registry(&root);
    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "doctor",
            "--profiles",
            profile_path.to_str().expect("utf8 profile path"),
        ])
        .output()
        .expect("run semantic-agent-hook doctor");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("doctor stdout");
    assert!(stdout.contains("[agent-doctor] status=ok"));
    assert!(stdout.contains("profiles=1"));
    assert!(stdout.contains("|profile language=rust provider=rs-harness"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_emits_decision_for_root_owned_rust_profile_registry() {
    let root = temp_project_root("hook-profile");
    let profile_path = write_root_owned_rust_profile_registry(&root);
    let mut child = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "hook",
            "--client",
            "codex",
            "pre-tool",
            "--profiles",
            profile_path.to_str().expect("utf8 profile path"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run semantic-agent-hook hook");
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
    assert_eq!(
        value["hookSpecificOutput"]["permissionDecisionReason"],
        "direct-source-read denied; route: rs-harness query --from-hook direct-source-read --selector src/lib.rs ."
    );
    assert!(context.contains("\"binary\":\"rs-harness\""));
    assert!(context.contains("\"src/lib.rs\""));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_can_emit_raw_decision_for_schema_tests() {
    let root = temp_project_root("hook-decision-profile");
    let profile_path = write_root_owned_rust_profile_registry(&root);
    let mut child = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "hook",
            "--client",
            "codex",
            "pre-tool",
            "--profiles",
            profile_path.to_str().expect("utf8 profile path"),
            "--emit",
            "decision",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run semantic-agent-hook hook");
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
    let root = temp_project_root("subagent-stop-profile");
    let profile_path = write_root_owned_rust_profile_registry(&root);
    let mut child = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "hook",
            "--client",
            "codex",
            "subagent-stop",
            "--profiles",
            profile_path.to_str().expect("utf8 profile path"),
            "--emit",
            "decision",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run semantic-agent-hook hook");
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
