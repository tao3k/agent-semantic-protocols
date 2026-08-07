//! Projects denied Hook policy decisions onto the Org-owned ChoicePlane.

const CHOICE_PLANE_COMMAND: &str = "asp session --agents choice-plane";
const CHOICE_PLANE_INSTRUCTION: &str = "Open the Agent ChoicePlane with `asp session --agents choice-plane`; it returns the direct registered `@name` Host action or the admitted Host registration choices.";

/// Reference the single Org-owned ChoicePlane without selecting or scheduling
/// a host Agent. The host owns subagent creation and executor lifecycle; Hook owns
/// only the local policy decision that interactive recovery is required.
pub(crate) fn materialize_org_choice_plane_reference(
    decision: &mut agent_semantic_hook::HookDecision,
) {
    if decision.decision == agent_semantic_hook::DecisionKind::Allow {
        return;
    }
    let interactive_recovery = decision.fields.contains_key("residentChildName")
        || decision.fields.contains_key("targetAgentName")
        || decision.fields.contains_key("completionReceipt")
        || decision
            .fields
            .get("requiredAction")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|action| action.contains("resident"));
    if !interactive_recovery {
        return;
    }

    materialize_org_choice_plane_fields(&mut decision.fields);
}

fn materialize_org_choice_plane_fields(
    fields: &mut std::collections::BTreeMap<String, serde_json::Value>,
) {
    for rust_owned_choice_field in [
        "transport",
        "residentName",
        "residentChildName",
        "targetAgentName",
        "targetAgentRole",
        "targetAgentSelectionSource",
        "canonicalTarget",
        "agentSessionAction",
        "receiptKind",
        "commandDigest",
        "completionReceipt",
        "requiredAction",
    ] {
        fields.remove(rust_owned_choice_field);
    }
    fields.insert(
        "nextAction".to_owned(),
        serde_json::Value::String(CHOICE_PLANE_INSTRUCTION.to_owned()),
    );
    fields.insert(
        "agentWindowCommand".to_owned(),
        serde_json::Value::String(CHOICE_PLANE_COMMAND.to_owned()),
    );
    fields.insert(
        "choicePlaneOwner".to_owned(),
        serde_json::Value::String("org-contract:agent-interactive".to_owned()),
    );
}

#[cfg(test)]
#[path = "../../tests/unit/command/hook_runtime_agent_session_dispatch.rs"]
mod tests;
