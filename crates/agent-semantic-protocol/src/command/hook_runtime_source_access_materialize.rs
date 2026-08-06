use agent_semantic_hook::{DecisionKind, HookDecision};

pub(crate) fn materialize_source_access_deny_message(decision: &mut HookDecision) {
    if decision.decision != DecisionKind::Deny {
        return;
    }
    let reason = serde_json::to_value(decision.reason_kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "source-access".to_owned());
    decision.message = format!(
        "ASP denied source access (`{reason}`). Open the Org-owned interactive ChoicePlane with `asp session --agents choice-plane` to select a parser-owned recovery route."
    );
}
