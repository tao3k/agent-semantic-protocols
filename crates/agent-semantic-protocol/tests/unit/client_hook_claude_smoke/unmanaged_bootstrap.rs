use super::{
    claude_fixture, codex_asp_query_payload, install_codex_hooks,
    run_codex_pre_tool_decision_with_env,
};

#[test]
fn codex_main_session_allows_recovery_without_asp_explore_registered() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    for command in [
        "asp org recall plans",
        "asp org capture --contract agent.plan.v1 --title plan --target-file plan.org --no-confirm",
    ] {
        let decision = run_codex_pre_tool_decision_with_env(
            &root,
            codex_asp_query_payload(command),
            &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000009")],
        );
        assert_eq!(
            decision["decision"].as_str(),
            Some("allow"),
            "command should not require asp-explore child: {command}\ndecision: {decision}"
        );
    }
}
