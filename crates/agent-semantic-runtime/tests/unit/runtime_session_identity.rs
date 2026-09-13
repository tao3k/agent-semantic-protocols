// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::agent_runtime_session_from_platform_ids;

#[test]
fn current_agent_runtime_session_prefers_codex_session_id_over_thread_id() {
    let session = agent_runtime_session_from_platform_ids(
        Some("session-id".to_string()),
        Some("thread-id".to_string()),
        None,
        None,
    )
    .expect("Codex identity should resolve");

    assert_eq!(session.client, "codex");
    assert_eq!(session.id, "session-id");
}

#[test]
fn current_agent_runtime_session_uses_codex_thread_id_as_fallback() {
    let session =
        agent_runtime_session_from_platform_ids(None, Some("thread-id".to_string()), None, None)
            .expect("Codex thread fallback should resolve");

    assert_eq!(session.client, "codex");
    assert_eq!(session.id, "thread-id");
}

#[test]
fn current_agent_runtime_session_prefers_local_claude_session_id() {
    let session = agent_runtime_session_from_platform_ids(
        None,
        None,
        Some("local-session".to_string()),
        Some("remote-session".to_string()),
    )
    .expect("Claude identity should resolve");

    assert_eq!(session.client, "claude-code");
    assert_eq!(session.id, "local-session");
}

#[test]
fn current_agent_runtime_session_rejects_cross_platform_ambiguity() {
    assert!(
        agent_runtime_session_from_platform_ids(
            Some("codex-session".to_string()),
            None,
            Some("claude-session".to_string()),
            None,
        )
        .is_none()
    );
}
