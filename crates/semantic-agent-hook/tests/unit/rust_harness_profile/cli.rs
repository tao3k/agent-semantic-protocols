use std::io::Write;
use std::process::{Command, Stdio};

mod install;
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
    assert_eq!(value["agentHookDecision"]["decision"], "deny");
    assert_eq!(
        value["agentHookDecision"]["reasonKind"],
        "direct-source-read"
    );
    assert_eq!(
        value["agentHookDecision"]["routes"][0]["binary"],
        "rs-harness"
    );
    assert_eq!(
        value["agentHookDecision"]["routes"][0]["argv"][3],
        "src/lib.rs"
    );
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
