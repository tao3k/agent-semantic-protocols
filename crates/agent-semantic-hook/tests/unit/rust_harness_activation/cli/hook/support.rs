use std::io::Write;
use std::path::Path;
use std::process::Stdio;

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
    let events = std::fs::read_to_string(hook_event_state_path(root)).expect("hook event state");
    let line = events
        .lines()
        .last()
        .expect("at least one recorded hook event");
    serde_json::from_str(line).expect("hook event JSON")
}

fn hook_event_state_path(root: &Path) -> std::path::PathBuf {
    let mut matches = Vec::new();
    collect_hook_event_state_paths(root, &mut matches);
    matches.sort();
    assert_eq!(matches.len(), 1, "hook event state paths: {matches:?}");
    matches.remove(0)
}

fn collect_hook_event_state_paths(dir: &Path, matches: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_hook_event_state_paths(&path, matches);
        } else if path.ends_with("live/hooks/state/events.jsonl") {
            matches.push(path);
        }
    }
}
