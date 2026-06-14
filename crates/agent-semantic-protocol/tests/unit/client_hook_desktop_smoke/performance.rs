use std::{
    fs,
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use agent_semantic_hook::{HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION};
use serde_json::{Value, json};

use super::{
    HOOK_STDIN_PERFORMANCE_GATE, assert_route_mentions, decision_from_stdout, hook_process_guard,
    spawn_hook, spawn_hook_event, temp_project_root, wait_for_hook_exit, write_hook_fixture,
};

#[test]
fn codex_desktop_hook_open_stdin_without_payload_exits_inside_gate() {
    let _guard = performance_gate_guard();
    let root = temp_project_root("codex-desktop-open-stdin-no-payload");
    write_hook_fixture(&root);

    let mut child = spawn_hook(&root);
    let _stdin_guard = child.stdin.take().expect("hook stdin");
    let (_stdout, elapsed) = wait_for_hook_exit(child, HOOK_STDIN_PERFORMANCE_GATE);

    assert!(
        elapsed < HOOK_STDIN_PERFORMANCE_GATE,
        "open stdin without payload should not wait for Codex's outer timeout; elapsed={elapsed:?}"
    );
}

#[test]
fn codex_desktop_hook_reads_payload_without_waiting_for_stdin_eof() {
    let _guard = performance_gate_guard();
    let root = temp_project_root("codex-desktop-open-stdin-with-payload");
    write_hook_fixture(&root);

    let payload = json!({
        "tool_name": "functions.exec_command",
        "tool_input": {
            "cmd": "sed -n '1,20p' src/lib.rs"
        }
    });
    let mut child = spawn_hook(&root);
    let mut stdin_guard = child.stdin.take().expect("hook stdin");
    stdin_guard
        .write_all(payload.to_string().as_bytes())
        .expect("write hook payload");
    let (stdout, elapsed) = wait_for_hook_exit(child, HOOK_STDIN_PERFORMANCE_GATE);

    assert!(
        elapsed < HOOK_STDIN_PERFORMANCE_GATE,
        "payload should be classified without waiting for stdin EOF; elapsed={elapsed:?}"
    );
    let decision = decision_from_stdout(&stdout);
    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "bulk-source-dump");
    assert_route_mentions(&decision, "src/lib.rs");
}

#[test]
fn codex_desktop_post_tool_open_stdin_without_payload_exits_inside_gate() {
    let _guard = performance_gate_guard();
    let root = temp_project_root("codex-desktop-post-tool-open-stdin-no-payload");
    write_hook_fixture(&root);

    let mut child = spawn_hook_event(&root, "post-tool");
    let _stdin_guard = child.stdin.take().expect("hook stdin");
    let (_stdout, elapsed) = wait_for_hook_exit(child, HOOK_STDIN_PERFORMANCE_GATE);

    assert!(
        elapsed < HOOK_STDIN_PERFORMANCE_GATE,
        "post-tool open stdin without payload should not wait for Codex's outer timeout; elapsed={elapsed:?}"
    );
}

#[test]
fn codex_desktop_post_tool_reads_payload_without_waiting_for_stdin_eof() {
    let _guard = performance_gate_guard();
    let root = temp_project_root("codex-desktop-post-tool-open-stdin-with-payload");
    write_hook_fixture(&root);

    let payload = json!({
        "tool_name": "functions.exec_command",
        "tool_input": {
            "cmd": "sed -n '1,20p' src/lib.rs"
        }
    });
    let mut child = spawn_hook_event(&root, "post-tool");
    let mut stdin_guard = child.stdin.take().expect("hook stdin");
    stdin_guard
        .write_all(payload.to_string().as_bytes())
        .expect("write hook payload");
    let (stdout, elapsed) = wait_for_hook_exit(child, HOOK_STDIN_PERFORMANCE_GATE);

    assert!(
        elapsed < HOOK_STDIN_PERFORMANCE_GATE,
        "post-tool payload should be classified without waiting for stdin EOF; elapsed={elapsed:?}"
    );
    serde_json::from_str::<Value>(stdout.trim()).expect("post-tool hook json envelope");
}

#[test]
fn codex_desktop_hook_large_stale_state_exits_inside_gate() {
    let _guard = performance_gate_guard();
    let root = temp_project_root("codex-desktop-large-stale-state");
    write_hook_fixture(&root);
    write_large_stale_hook_state(&root);

    let payload = json!({
        "tool_name": "functions.exec_command",
        "tool_input": {
            "cmd": "sed -n '1,20p' src/lib.rs"
        }
    });
    let mut child = spawn_hook(&root);
    let mut stdin_guard = child.stdin.take().expect("hook stdin");
    stdin_guard
        .write_all(payload.to_string().as_bytes())
        .expect("write hook payload");
    let (stdout, elapsed) = wait_for_hook_exit(child, HOOK_STDIN_PERFORMANCE_GATE);

    assert!(
        elapsed < HOOK_STDIN_PERFORMANCE_GATE,
        "large stale hook state should not be scanned unboundedly; elapsed={elapsed:?}"
    );
    let decision = decision_from_stdout(&stdout);
    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "bulk-source-dump");
    assert_eq!(decision["fields"]["denyReplay"], "record");
}

#[test]
fn codex_desktop_hook_large_recent_nonmatching_state_exits_inside_gate() {
    let _guard = performance_gate_guard();
    let root = temp_project_root("codex-desktop-large-recent-nonmatching-state");
    write_hook_fixture(&root);
    write_large_recent_nonmatching_hook_state(&root);

    let payload = json!({
        "tool_name": "functions.exec_command",
        "tool_input": {
            "cmd": "sed -n '1,20p' src/lib.rs"
        }
    });
    let mut child = spawn_hook(&root);
    let mut stdin_guard = child.stdin.take().expect("hook stdin");
    stdin_guard
        .write_all(payload.to_string().as_bytes())
        .expect("write hook payload");
    let (stdout, elapsed) = wait_for_hook_exit(child, HOOK_STDIN_PERFORMANCE_GATE);

    assert!(
        elapsed < HOOK_STDIN_PERFORMANCE_GATE,
        "large recent nonmatching hook state should not be scanned unboundedly; elapsed={elapsed:?}"
    );
    let decision = decision_from_stdout(&stdout);
    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "bulk-source-dump");
    assert_eq!(decision["fields"]["denyReplay"], "record");
}

fn performance_gate_guard() -> std::sync::MutexGuard<'static, ()> {
    hook_process_guard()
}

fn write_large_stale_hook_state(root: &Path) {
    write_large_hook_state(root, 1, "stale");
}

fn write_large_recent_nonmatching_hook_state(root: &Path) {
    let recorded_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis()
        .try_into()
        .expect("unix ms fits u64");
    write_large_hook_state(root, recorded_at_unix_ms, "recent-nonmatching");
}

fn write_large_hook_state(root: &Path, recorded_at_unix_ms: u64, deny_replay_key: &str) {
    let state_path = root
        .join(".cache/agent-semantic-protocol/hooks")
        .join("events.jsonl");
    let mut file = fs::File::create(&state_path).expect("create hook state");
    let padding = "x".repeat(384);
    let line = json!({
        "schemaId": "agent.semantic-protocols.hook.event",
        "schemaVersion": "1",
        "protocolId": HOOK_PROTOCOL_ID,
        "protocolVersion": HOOK_PROTOCOL_VERSION,
        "recordedAtUnixMs": recorded_at_unix_ms,
        "platform": "codex",
        "event": "pre-tool",
        "decision": "deny",
        "reasonKind": "bulk-source-dump",
        "languageIds": ["rust"],
        "subject": {
            "toolName": "functions.exec_command",
            "command": "sed -n '1,20p' src/lib.rs",
            "paths": ["src/lib.rs"]
        },
        "routeKinds": ["query"],
        "fields": {},
        "denyReplayKey": deny_replay_key,
        "padding": padding
    })
    .to_string();
    for _ in 0..80_000 {
        file.write_all(line.as_bytes()).expect("write hook state");
        file.write_all(b"\n").expect("write hook state newline");
    }
}
