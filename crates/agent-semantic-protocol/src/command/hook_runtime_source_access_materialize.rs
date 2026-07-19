use super::hook_selected_resident_execution;
use agent_semantic_hook::{DecisionKind, HookDecision};

pub(super) fn materialize_source_access_deny_message(
    decision: &mut HookDecision,
    hook_config: &agent_semantic_hook::ClientHookConfig,
) {
    if decision.decision != DecisionKind::Deny {
        return;
    }
    if hook_selected_resident_execution(decision) {
        return;
    }
    decision
        .fields
        .entry("residentChildName".to_string())
        .or_insert_with(|| {
            serde_json::Value::String(hook_config.resident_asp_explore_child_name().to_string())
        });
    let resident_child_name = hook_config.resident_asp_explore_child_name();
    if let Ok(serialized) = serde_json::to_value(&*decision)
        && let Some(message) =
            super::super::hook_runtime_source_access::compact_root_source_access_message(
                &serialized,
                resident_child_name,
            )
    {
        decision.message = message;
    }
}
