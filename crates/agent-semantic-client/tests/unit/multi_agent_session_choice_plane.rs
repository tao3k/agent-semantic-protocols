use super::current_codex_session_id;

#[test]
fn current_codex_thread_is_authoritative_over_a_prior_hook_session() {
    let current = current_codex_session_id(
        Some("current-codex-session"),
        Some("secondary-codex-session"),
    )
    .expect("current Codex identity");

    assert_eq!(current, "current-codex-session");
    assert_ne!(current, "prior-hook-session");
}

#[test]
fn codex_choice_plane_never_uses_a_hook_event_as_session_fallback() {
    let error = current_codex_session_id(None, None)
        .expect_err("a Hook event is route evidence, not session authority");

    assert_eq!(
        error,
        "agent-session-codex-id-required: current Codex session identity is required; a prior Hook event cannot supply it"
    );
}
