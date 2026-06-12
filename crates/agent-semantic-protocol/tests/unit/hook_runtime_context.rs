#[path = "../../src/command/hook_runtime_context.rs"]
mod hook_runtime_context;

use serde_json::json;

#[test]
fn payload_subagent_detection_accepts_explicit_context_flags() {
    assert!(hook_runtime_context::payload_indicates_subagent_context(
        &json!({"isSubagent": true})
    ));
    assert!(hook_runtime_context::payload_indicates_subagent_context(
        &json!({"parentAgentId": "agent-123"})
    ));
    assert!(hook_runtime_context::payload_indicates_subagent_context(
        &json!({"thread": {"threadKind": "child-agent"}})
    ));
}

#[test]
fn payload_subagent_detection_ignores_main_thread_payloads() {
    assert!(!hook_runtime_context::payload_indicates_subagent_context(
        &json!({
            "session_id": "session-123",
            "tool_name": "Bash",
            "tool_input": {
                "command": "asp rust search pipe 'subagent hook' --workspace . --view seeds"
            }
        })
    ));
    assert!(!hook_runtime_context::payload_indicates_subagent_context(
        &json!({"isSubagent": false})
    ));
}
