use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use agent_semantic_runtime::ensure_project_hook_state_dir;
use serde_json::Value;

use crate::rust_harness_activation::support::{asp_command, root_owned_rust_activation_json};

pub(super) fn run_hook_decision(root: &Path, event: &str, payload: Value) -> Value {
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
    let mut child = asp_command()
        .current_dir(root)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "hook",
            "--client",
            "codex",
            event,
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
        .write_all(payload.to_string().as_bytes())
        .expect("write hook payload");
    let output = child.wait_with_output().expect("wait for hook command");
    assert!(
        output.status.success(),
        "hook stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("hook decision JSON")
}

pub(super) fn last_hook_event(root: &Path) -> Value {
    let state_dir = ensure_project_hook_state_dir(root).expect("hook state dir");
    let events = std::fs::read_to_string(state_dir.join("events.jsonl")).expect("hook event state");
    let line = events
        .lines()
        .last()
        .expect("at least one recorded hook event");
    serde_json::from_str(line).expect("hook event JSON")
}
