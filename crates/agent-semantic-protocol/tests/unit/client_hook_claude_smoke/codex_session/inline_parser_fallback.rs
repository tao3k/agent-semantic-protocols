use super::{
    assert_configured_asp_explore_dispatch, claude_fixture, codex_asp_query_payload,
    install_codex_hooks, run_codex_pre_tool_decision_with_env,
};

const ROOT_SESSION_ID: &str = "019f5c84-0000-7000-8000-000000000301";

#[test]
fn explicit_inline_fallback_flag_does_not_bypass_resident_lifecycle() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload(
            "ASP_INLINE_PARSER_FALLBACK=1 direnv exec . asp rust search lexical lifecycle owner tests --workspace . --view seeds",
        ),
        &[("CODEX_THREAD_ID", ROOT_SESSION_ID)],
    );

    assert_configured_asp_explore_dispatch(&decision);
    assert!(decision["fields"].get("executionTransport").is_none());
    assert!(decision["fields"].get("degraded").is_none());
    assert!(decision["fields"].get("executionCommandDigest").is_none());
}

#[test]
fn parser_owned_search_without_inline_opt_in_remains_denied() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload(
            "direnv exec . asp rust search lexical lifecycle owner tests --workspace . --view seeds",
        ),
        &[("CODEX_THREAD_ID", ROOT_SESSION_ID)],
    );

    assert_configured_asp_explore_dispatch(&decision);
}

#[test]
fn inline_fallback_opt_in_never_authorizes_raw_shell_search() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("ASP_INLINE_PARSER_FALLBACK=1 rg -n lifecycle crates"),
        &[("CODEX_THREAD_ID", ROOT_SESSION_ID)],
    );

    assert_eq!(decision["decision"], "deny", "{decision}");
    assert_ne!(
        decision["fields"]["agentSessionAction"],
        "inline-parser-fallback"
    );
    assert_ne!(decision["reasonKind"], "none");
}
