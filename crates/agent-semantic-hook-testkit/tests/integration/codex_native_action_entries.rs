use serde_json::Value;
use std::collections::BTreeSet;

const CODEX_PLUGIN_HOOKS: &str = include_str!("../../../../asp-codex-plugin/hooks/hooks.json");
const HOOK_CONFIG: &str =
    include_str!("../../../agent-semantic-config/templates/hooks/config.toml");

const CODEX_APP_MATCHERS: &[&str] = &[
    "mcp__codex_app__automation_update",
    "mcp__codex_app__create_thread",
    "mcp__codex_app__fork_thread",
    "mcp__codex_app__list_threads",
    "mcp__codex_app__list_projects",
    "mcp__codex_app__list_archived_threads",
    "mcp__codex_app__read_thread",
    "mcp__codex_app__read_thread_terminal",
    "mcp__codex_app__load_workspace_dependencies",
    "mcp__codex_app__send_message_to_thread",
    "mcp__codex_app__wait_threads",
    "mcp__codex_app__handoff_thread",
    "mcp__codex_app__get_handoff_status",
    "mcp__codex_app__navigate_to_codex_page",
    "mcp__codex_app__open_in_codex",
    "mcp__codex_app__set_thread_archived",
    "mcp__codex_app__set_thread_pinned",
    "mcp__codex_app__set_thread_title",
    "mcp__codex_app__share_thread",
];

const HOST_OWNED_SENSITIVE_TOOLS: &[&str] = &[
    "mcp__codex_app__consume_usage_reset",
    "mcp__codex_app__uninstall_plugin",
];

fn pre_tool_entries() -> Vec<&'static Value> {
    let document: &'static Value = Box::leak(Box::new(
        serde_json::from_str(CODEX_PLUGIN_HOOKS).expect("Codex hooks.json must be valid JSON"),
    ));
    document["hooks"]["PreToolUse"]
        .as_array()
        .expect("Codex plugin must declare PreToolUse entries")
        .iter()
        .collect()
}

#[test]
fn codex_plugin_has_one_policy_plane_and_no_permission_request_recheck() {
    let document: Value =
        serde_json::from_str(CODEX_PLUGIN_HOOKS).expect("Codex hooks.json must be valid JSON");
    assert!(document["hooks"]["PreToolUse"].is_array());
    assert!(
        document["hooks"].get("PermissionRequest").is_none(),
        "PermissionRequest cannot re-evaluate PreTool policy or block ASP_NO_AGENT publication"
    );
}

#[test]
fn codex_pre_tool_entries_follow_the_host_tool_name_schema() {
    let entries = pre_tool_entries();
    let matchers = entries
        .iter()
        .map(|entry| {
            entry["matcher"]
                .as_str()
                .expect("every PreToolUse entry must declare one matcher")
        })
        .collect::<Vec<_>>();

    let expected = ["^apply_patch$", "Bash", "spawn_agent"]
        .into_iter()
        .chain(CODEX_APP_MATCHERS.iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(matchers, expected);
    assert_eq!(
        matchers.iter().copied().collect::<BTreeSet<_>>().len(),
        matchers.len(),
        "one Host action family must have one matcher entry"
    );
    assert!(!matchers.contains(&"Read"));
    assert!(!matchers.contains(&"NotebookEdit"));
    assert!(!matchers.contains(&"MCP"));
    assert!(!matchers.contains(&"Agent"));
    for tool_name in HOST_OWNED_SENSITIVE_TOOLS {
        assert!(
            !matchers.contains(tool_name),
            "sensitive Host-owned tool must not enter ASP Hook: {tool_name}"
        );
        assert!(
            !HOOK_CONFIG.contains(tool_name),
            "sensitive Host-owned tool must not enter ASP Config: {tool_name}"
        );
    }
}

#[test]
fn canonical_apply_patch_selects_one_hook_and_forwards_the_same_identity() {
    let entries = pre_tool_entries();
    let edit = entries
        .iter()
        .find(|entry| entry["matcher"] == "^apply_patch$")
        .expect("the canonical Codex apply_patch action must be installed");
    let hooks = edit["hooks"]
        .as_array()
        .expect("the edit matcher must own command hooks");

    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0]["type"], "command");
    let command = hooks[0]["command"]
        .as_str()
        .expect("the edit hook must declare a command");
    assert!(command.contains("--host-match apply_patch"));
    assert!(!command.contains("--host-match Write"));
    assert!(!command.contains("--host-match Edit"));
}

#[test]
fn config_dsl_consumes_the_same_matcher_without_duplicate_action_fields() {
    assert!(HOOK_CONFIG.contains("matcher = \"apply_patch\""));
    assert!(!HOOK_CONFIG.contains("matcher = \"Edit|Write\""));
    assert!(!HOOK_CONFIG.contains("matcher = \"apply_patch|Write|Edit\""));
    assert!(!HOOK_CONFIG.contains("matcher = \"Read\""));
    assert!(!HOOK_CONFIG.contains("NotebookEdit"));
    assert!(!HOOK_CONFIG.contains("hostAction"));
    assert!(!HOOK_CONFIG.contains("semanticAction"));

    let deployed = pre_tool_entries()
        .iter()
        .map(|entry| entry["matcher"].as_str().expect("matcher string"))
        .collect::<BTreeSet<_>>();
    let configured = HOOK_CONFIG
        .lines()
        .filter_map(|line| line.strip_prefix("matcher = \"")?.strip_suffix('"'))
        .flat_map(|expression| expression.split('|'))
        .collect::<BTreeSet<_>>();
    assert!(
        configured.iter().all(|matcher| {
            deployed.contains(matcher)
                || (*matcher == "apply_patch" && deployed.contains("^apply_patch$"))
        }),
        "Config aliases must be forwarded by hooks.json: {configured:?}"
    );
}

#[test]
fn codex_app_tools_use_exact_native_matchers_not_an_mcp_transport_wildcard() {
    let entries = pre_tool_entries();
    assert!(entries.iter().all(|entry| entry["matcher"] != "^mcp__.*$"));
    for matcher in CODEX_APP_MATCHERS {
        let matches = entries
            .iter()
            .filter(|entry| entry["matcher"] == *matcher)
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "{matcher} must own one exact Host entry");
        assert_eq!(matches[0]["hooks"][0]["type"], "command");
        let command = matches[0]["hooks"][0]["command"]
            .as_str()
            .expect("exact Codex App matcher must declare a command");
        assert!(command.contains(&format!("--host-match {matcher}")));
        assert!(!command.contains("--host-match-prefix"));
    }
}

#[test]
fn config_validation_accepts_only_declarative_host_aliases() {
    for invalid in ["[", "(", "mcp__(", "^mcp__.*$"] {
        let error = agent_semantic_config::validate_codex_host_matcher_expression(invalid)
            .expect_err("a non-declarative Host matcher must fail closed");
        assert!(
            error.contains("uses unsupported Host matcher"),
            "unexpected validation error for {invalid}: {error}"
        );
        assert!(error.contains(invalid));
    }

    for valid_expression in [
        "",
        "*",
        "Bash",
        "apply_patch",
        "Edit|Write",
        "mcp__filesystem__read_file",
        "startup|resume|clear|compact",
        "manual|auto",
    ] {
        agent_semantic_config::validate_codex_host_matcher_expression(valid_expression)
            .expect("an official Codex matcher expression must be accepted");
    }
}
