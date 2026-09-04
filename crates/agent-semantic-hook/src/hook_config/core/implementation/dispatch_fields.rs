//! Projects configured Host dispatch metadata onto every Hook decision path.

use sha2::Digest;
use sha2::Sha256;

use super::compiled_rule::CompiledRuleDispatch;
use crate::tool_action::ToolAction;

pub(super) fn extend_dispatch_fields(
    fields: &mut std::collections::BTreeMap<String, serde_json::Value>,
    dispatch: Option<&CompiledRuleDispatch>,
    platform: &str,
    action: &ToolAction,
) {
    let Some(dispatch) = dispatch else {
        return;
    };
    for (field, value) in [
        ("transport", dispatch.transport.as_str()),
        ("targetAgent", dispatch.target_agent.as_str()),
        ("agentSessionAction", "dispatch-registered-agent"),
        ("receiptKind", dispatch.receipt_kind.as_str()),
        ("targetAgentSelectionSource", "config-agent-route"),
    ] {
        fields.insert(
            field.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
    fields.insert(
        "targetAgentSymbol".to_string(),
        serde_json::Value::String(
            dispatch
                .calling
                .symbol(platform, dispatch.target_agent.as_str()),
        ),
    );
    if let Some(command) = action.command.as_deref() {
        fields.insert(
            "commandDigest".to_string(),
            serde_json::Value::String(format!("sha256:{:x}", Sha256::digest(command.as_bytes()))),
        );
    }
    for (field, value) in [
        ("requiredAction", "collaboration.spawn_agent"),
        ("nextAction", "spawn-configured-agent"),
        ("collaborationNamespace", "collaboration"),
        ("collaborationTool", "spawn_agent"),
    ] {
        fields.insert(
            field.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
}
