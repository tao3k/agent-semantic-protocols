use serde_json::{Value, json};

use super::{
    classify, default_client_config_template, load_policy_with_configured_capabilities, registry,
    shell, temp_project_root,
};

fn shell_surface(tool_name: &str, field: &str, command: &str) -> Value {
    json!({
        "tool_name": tool_name,
        "tool_input": {field: command},
    })
}

fn normalized_agent_action(decision: &agent_semantic_hook::HookDecision) -> &Value {
    decision.fields.get("agentAction").unwrap_or(&Value::Null)
}

#[path = "branch_coverage/scenarios.rs"]
mod scenarios;
