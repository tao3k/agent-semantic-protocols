use super::{
    claude_fixture, codex_asp_query_payload, install_codex_hooks, register_asp_explore_session,
    run_codex_pre_tool_decision_with_env, write_codex_asp_explore_rollout,
};

#[test]
fn profile_valid_current_resident_bypasses_stale_same_name_registry_owner() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    let root_session_id = "019f126d-0000-7000-8000-000000000051";
    let stale_child_id = "019f126d-0000-7000-8000-000000000151";
    let current_child_id = "019f126d-0000-7000-8000-000000000251";
    install_codex_hooks(&root, &codex_home);
    register_asp_explore_session(&root, root_session_id, stale_child_id);
    write_codex_asp_explore_rollout(&root, root_session_id, current_child_id, "gpt-5.4-mini");

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload(
            "asp rust search owner src/lib.rs items --workspace . --view seeds",
        ),
        &[
            ("CODEX_THREAD_ID", current_child_id),
            ("ASP_ROOT_SESSION_ID", root_session_id),
        ],
    );

    assert_eq!(decision["decision"].as_str(), Some("allow"), "{decision}");
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("active-resident-child")
    );
    assert_eq!(decision["fields"]["routingTerminal"].as_bool(), Some(true));
    assert_eq!(
        decision["fields"]["redispatchAllowed"].as_bool(),
        Some(false)
    );
}

#[test]
fn exact_owner_search_executes_on_main_parser_lane() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    let root_session_id = "019f126d-0000-7000-8000-000000000052";
    install_codex_hooks(&root, &codex_home);
    register_asp_explore_session(
        &root,
        root_session_id,
        "019f126d-0000-7000-8000-000000000152",
    );

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload(
            "asp rust search owner src/lib.rs items --workspace . --view seeds",
        ),
        &[("CODEX_THREAD_ID", root_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("allow"), "{decision}");
    assert_ne!(
        decision["reasonKind"].as_str(),
        Some("asp-reasoning-routed"),
        "{decision}"
    );
}
