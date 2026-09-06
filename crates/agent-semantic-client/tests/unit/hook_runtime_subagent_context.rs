// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::{
    apply_codex_subagent_context, codex_rollout_is_subagent,
    payload_has_complete_typed_agent_identity, payload_indicates_subagent_context,
};

#[test]
fn root_rollout_is_never_enriched_as_a_subagent() {
    assert!(!codex_rollout_is_subagent("root", "root", "parent"));
    assert!(!codex_rollout_is_subagent("root", "root", ""));
    assert!(!codex_rollout_is_subagent("parent", "root", "parent"));
    assert!(codex_rollout_is_subagent("child", "root", "parent"));
}

#[test]
fn typed_rollout_identity_normalizes_subagent_policy_facts() {
    let mut payload = serde_json::json!({"tool_name": "Bash"});

    apply_codex_subagent_context(
        &mut payload,
        "child-session",
        "root-session",
        Some("parent-session"),
        "asp_testing",
    );

    assert!(payload_indicates_subagent_context(&payload));
    assert_eq!(payload["is_subagent"], true);
    assert_eq!(payload["agent_id"], "child-session");
    assert_eq!(payload["agent_type"], "asp_testing");
    assert_eq!(payload["parent_session_id"], "parent-session");
}

#[test]
fn typed_rollout_identity_uses_root_when_direct_parent_is_absent() {
    let mut payload = serde_json::json!({});

    apply_codex_subagent_context(
        &mut payload,
        "child-session",
        "root-session",
        None,
        "asp_explorer",
    );

    assert_eq!(payload["parent_session_id"], "root-session");
}

#[test]
fn subagent_flag_without_typed_identity_is_enriched_to_dispatch_fixed_point() {
    let mut payload = serde_json::json!({
        "is_subagent": true,
        "agent_id": "",
        "agent_type": "",
    });
    assert!(payload_indicates_subagent_context(&payload));
    assert!(!payload_has_complete_typed_agent_identity(&payload));

    apply_codex_subagent_context(
        &mut payload,
        "testing-child",
        "root-session",
        Some("parent-session"),
        "asp_testing",
    );

    assert!(payload_has_complete_typed_agent_identity(&payload));
    assert_eq!(payload["agent_id"], "testing-child");
    assert_eq!(payload["agent_type"], "asp_testing");
}
