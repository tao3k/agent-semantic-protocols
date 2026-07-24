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
    super::support::write_default_client_hook_config(&root);
    let activation_path = write_root_owned_rust_activation(&root);
    let output = asp_command()
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env("CODEX_HOME", root.join(".codex-home"))
        .args([
            "hook",
            "doctor",
            "--activation",
            activation_path.to_str().expect("utf8 activation path"),
            root.to_str().expect("utf8 project root"),
        ])
        .output()
        .expect("run agent-semantic-protocol doctor");

    let stdout = String::from_utf8(output.stdout).expect("doctor stdout");
    let stderr = String::from_utf8(output.stderr).expect("doctor stderr");
    assert!(
        output.status.success(),
        "doctor failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("[agent-doctor] status=ok")
            || stdout.contains("[agent-doctor] status=warning")
    );
    assert!(stdout.contains("providers=1"));
    assert!(stdout.contains("clientConfigStatus="));
    assert!(stdout.contains("classifierProbe="));
    assert!(stdout.contains("classifierReason="));
    assert!(stdout.contains("enforcement="), "{stdout}");
    assert!(stdout.contains("enforcementProbe="));
    assert!(stdout.contains("enforcementReason="));
    assert!(stdout.contains("|provider language=rust provider=rs-harness"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_doctor_reports_deny_for_codex_exec_command_source_dump() {
    let root = temp_project_root("doctor-classifier-probe");
    let activation_path = write_root_owned_rust_activation(&root);
    write_default_client_hook_config(&root);
    let output = asp_command()
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .args([
            "hook",
            "doctor",
            "--activation",
            activation_path.to_str().expect("utf8 activation path"),
            root.to_str().expect("utf8 project root"),
        ])
        .output()
        .expect("run agent-semantic-protocol doctor");

    let stdout = String::from_utf8(output.stdout).expect("doctor stdout");
    let stderr = String::from_utf8(output.stderr).expect("doctor stderr");
    assert!(
        output.status.success(),
        "doctor failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("[agent-doctor] status=ok")
            || stdout.contains("[agent-doctor] status=warning")
    );
    assert!(stdout.contains("clientConfigStatus=ok"));
    assert!(stdout.contains("binaryContractStatus="));
    assert!(stdout.contains("binaryContractFingerprint=hook-client-v1-"));
    assert!(stdout.contains("activeContractFingerprint="));
    assert!(stdout.contains("classifierProbe=deny"));
    assert!(stdout.contains("classifierReason=bulk-source-dump"));
    assert!(stdout.contains("enforcement="), "{stdout}");
    assert!(stdout.contains("enforcementProbe="));
    assert!(stdout.contains("enforcementReason="));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_emits_deny_for_explicit_read_schema_tests() {
    let root = temp_project_root("hook-decision-activation");
    let activation_path = write_root_owned_rust_activation(&root);
    let mut child = asp_command()
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
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
fn cli_hook_allows_unmanaged_subagent_stop_without_search_receipt() {
    let root = temp_project_root("subagent-stop-activation");
    let activation_path = write_root_owned_rust_activation(&root);
    let mut child = asp_command()
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
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
    assert_eq!(value["decision"], "allow");
    assert_eq!(value["reasonKind"], "none");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}
