use serde_json::Value;
use std::collections::BTreeSet;

const CODEX_PLUGIN_HOOKS: &str = include_str!("../../../asp-codex-plugin/hooks/hooks.json");
const HOOK_CONFIG: &str = include_str!("../../agent-semantic-config/templates/hooks/config.toml");

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

    assert_eq!(
        matchers,
        vec!["^apply_patch$", "Bash", "spawn_agent", "^mcp__.*$"]
    );
    assert_eq!(
        matchers.iter().copied().collect::<BTreeSet<_>>().len(),
        matchers.len(),
        "one Host action family must have one matcher entry"
    );
    assert!(!matchers.contains(&"Read"));
    assert!(!matchers.contains(&"NotebookEdit"));
    assert!(!matchers.contains(&"MCP"));
    assert!(!matchers.contains(&"Agent"));
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
    assert!(HOOK_CONFIG.contains("matcher = \"^apply_patch$\""));
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
        .collect::<BTreeSet<_>>();
    assert!(
        configured.is_subset(&deployed),
        "Config matcher expressions must be reachable through hooks.json: {configured:?}"
    );
}

#[test]
fn mcp_is_a_dynamic_tool_name_pattern_not_a_handler_action_alias() {
    let entries = pre_tool_entries();
    let mcp = entries
        .iter()
        .find(|entry| entry["matcher"] == "^mcp__.*$")
        .expect("dynamic MCP tool identities must have one bounded matcher");

    assert_eq!(mcp["hooks"][0]["type"], "command");
    assert!(
        mcp["hooks"][0]["command"]
            .as_str()
            .expect("the MCP matcher must declare a command")
            .contains("--host-match-prefix mcp__")
    );
}

#[test]
fn config_validation_mirrors_codex_matcher_grammar() {
    for invalid in ["[", "(", "mcp__("] {
        let error = agent_semantic_config::validate_codex_host_matcher_expression(invalid)
            .expect_err("an invalid Codex matcher regex must fail closed");
        assert!(
            error.contains("uses invalid Codex Host matcher"),
            "unexpected validation error for {invalid}: {error}"
        );
        assert!(error.contains(invalid));
    }

    for valid_expression in [
        "",
        "*",
        "Bash",
        "^apply_patch$",
        "Edit|Write",
        "mcp__filesystem__read_file",
        "^mcp__.*$",
        "startup|resume|clear|compact",
        "manual|auto",
    ] {
        agent_semantic_config::validate_codex_host_matcher_expression(valid_expression)
            .expect("an official Codex matcher expression must be accepted");
    }
}
