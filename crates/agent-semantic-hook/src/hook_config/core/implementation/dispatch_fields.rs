//! Projects configured Host dispatch metadata onto every Hook decision path.

use sha2::{Digest, Sha256};

use super::compiled_rule::CompiledRuleDispatch;
use crate::tool_action::ToolAction;

pub(super) fn extend_dispatch_fields(
    fields: &mut std::collections::BTreeMap<String, serde_json::Value>,
    dispatch: Option<&CompiledRuleDispatch>,
    action: &ToolAction,
) {
    let Some(dispatch) = dispatch else {
        return;
    };
    for (field, value) in [
        ("transport", dispatch.transport.as_str()),
        ("residentName", dispatch.resident_name.as_str()),
        (
            "targetAgentName",
            dispatch.resident_codex_agent_name.as_str(),
        ),
        ("targetAgentRole", dispatch.resident_role.as_str()),
        (
            "targetAgentDescription",
            dispatch.resident_description.as_str(),
        ),
        ("agentSessionAction", "dispatch-configured-resident"),
        ("receiptKind", dispatch.receipt_kind.as_str()),
        ("targetAgentSelectionSource", "hook-config-rule-dispatch"),
    ] {
        fields.insert(
            field.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
    if let Some(command) = action.command.as_deref() {
        fields.insert(
            "commandDigest".to_string(),
            serde_json::Value::String(format!("sha256:{:x}", Sha256::digest(command.as_bytes()))),
        );
    }
    for (field, value) in [
        (
            "requiredAction",
            "open-org-interactive-resident-agent-window",
        ),
        ("nextAction", "run-asp-session-agent-window"),
        ("agentWindowCommand", "asp session --agents choice-plane"),
        ("choicePlaneOwner", "org-contract:agent-interactive"),
    ] {
        fields.insert(
            field.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
}
