// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[tokio::test]
async fn agent_session_registry_call_is_bounded() {
    let error = super::bounded_agent_session_registry_call(
        std::time::Duration::from_millis(1),
        std::future::pending::<Result<(), String>>(),
    )
    .await
    .expect_err("pending registry call must time out");
    assert_eq!(
        error,
        "Runtime Server agent-session registry IPC timed out after 1ms"
    );
}
