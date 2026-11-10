// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::is_source_access_replay_reason;

use crate::event_replay::structured_source_read_repeated_message;

#[test]
fn structured_source_read_uses_source_access_replay_family() {
    assert!(is_source_access_replay_reason(Some(
        "structured-source-read"
    )));
    assert!(!is_source_access_replay_reason(Some("agent-search-json")));
}

#[test]
fn repeated_structured_source_read_preserves_configured_recovery() {
    let message = structured_source_read_repeated_message(
        "structured-source-read",
        "Use jq with bounded-path-v1; do not retry raw Read.",
        "source-access:abc",
    )
    .expect("structured replay message");

    assert!(message.contains("Use jq with bounded-path-v1"), "{message}");
    assert!(message.contains("source-access:abc"), "{message}");
    assert!(!message.contains("resident-child"), "{message}");
}
