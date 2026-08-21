use serde_json::json;

use crate::hook_testkit::{fixture_root, run_hook, write_fixture};

#[test]
fn explicit_no_agent_bypass_precedes_registered_language_reasoning_search() {
    let root = fixture_root();
    let state_home = root.join(".agent-semantic-protocols");
    write_fixture(&root, &state_home);
    let activation = crate::state_home_fixture::canonical_activation_path(&root, &state_home);
    let decision = run_hook(
        &root,
        &state_home,
        &activation,
        json!({
            "tool_name": "exec_command",
            "tool_input": {
                "cmd": "ASP_NO_AGENT=1 asp rust search owner src/lib.rs items --query install --workspace . --view seeds"
            }
        }),
    );
    assert_eq!(decision["decision"], "allow", "{decision}");
    assert_eq!(
        decision["fields"]["configRuleId"],
        "allow-explicit-no-agent-host-bypass",
        "{decision}"
    );
    std::fs::remove_dir_all(root).expect("remove language bypass Hook fixture");
}

#[test]
fn registered_language_reasoning_search_routes_to_explorer_without_host_bypass() {
    let root = fixture_root();
    let state_home = root.join(".agent-semantic-protocols");
    write_fixture(&root, &state_home);
    let activation = crate::state_home_fixture::canonical_activation_path(&root, &state_home);
    let decision = run_hook(
        &root,
        &state_home,
        &activation,
        json!({
            "tool_name": "exec_command",
            "tool_input": {
                "cmd": "asp rust search owner src/lib.rs items --query install --workspace . --view seeds"
            }
        }),
    );
    assert_eq!(decision["decision"], "deny", "{decision}");
    assert_eq!(
        decision["fields"]["configRuleId"],
        "registered-asp-reasoning-search",
        "{decision}"
    );
    assert_eq!(decision["fields"]["targetAgentName"], "asp_explorer", "{decision}");
    std::fs::remove_dir_all(root).expect("remove language dispatch Hook fixture");
}
