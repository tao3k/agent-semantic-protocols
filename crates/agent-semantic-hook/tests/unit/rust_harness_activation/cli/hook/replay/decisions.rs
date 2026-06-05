use std::io::Write;
use std::process::Stdio;

use serde_json::{Value, json};

use crate::rust_harness_activation::support::{
    asp_command, root_owned_rust_activation_json, temp_project_root,
};

use crate::rust_harness_activation::cli::hook::support::{last_hook_event, run_hook_decision};

#[test]
fn cli_hook_replay_records_allow_decision_for_exec_command_post_tool() {
    let root = temp_project_root("hook-exec-allow-post-tool");
    let decision = run_hook_decision(
        &root,
        "post-tool",
        json!({
            "toolName": "functions.exec_command",
            "toolInput": {"cmd": "cargo test -p agent-semantic-hook classifier::routes"}
        }),
    );

    assert_eq!(decision["decision"], "allow");
    assert_eq!(decision["reasonKind"], "none");
    assert_eq!(decision["event"], "post-tool");
    assert_eq!(decision["subject"]["toolName"], "functions.exec_command");
    assert_eq!(
        decision["subject"]["command"],
        "cargo test -p agent-semantic-hook classifier::routes"
    );
    let event = last_hook_event(&root);
    assert_eq!(event["event"], "post-tool");
    assert_eq!(event["decision"], "allow");
    assert_eq!(event["reasonKind"], "none");
    assert_eq!(event["subject"]["toolName"], "functions.exec_command");

    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_fails_open_on_invalid_payload_json() {
    let root = temp_project_root("hook-invalid-payload");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
    let mut child = asp_command()
        .args([
            "hook",
            "--client",
            "codex",
            "pre-tool",
            "--emit",
            "decision",
            "--activation",
            activation_path.to_str().expect("utf8 activation path"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run asp hook");
    child
        .stdin
        .as_mut()
        .expect("hook stdin")
        .write_all(b"{not-json")
        .expect("write invalid hook payload");

    let output = child.wait_with_output().expect("wait for hook command");

    assert!(
        output.status.success(),
        "hook stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decision: Value = serde_json::from_slice(&output.stdout).expect("hook decision JSON");
    assert_eq!(decision["decision"], "allow");
    assert_eq!(decision["reasonKind"], "none");
    assert!(
        decision["message"]
            .as_str()
            .unwrap()
            .contains("invalid hook payload JSON")
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}
