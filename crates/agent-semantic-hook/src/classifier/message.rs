//! Refines agent-facing recovery text without changing policy decisions.

use crate::DecisionKind;
use crate::HookDecision;

pub fn materialize_source_access_deny_message(decision: &mut HookDecision) {
    if decision.decision != DecisionKind::Deny {
        return;
    }
    if !decision.message.trim().is_empty() {
        if decision.message.contains("{{languageId}}")
            && let Some(language_id) = decision.language_ids.first()
        {
            decision.message = decision
                .message
                .replace("{{languageId}}", language_id.as_str());
        }
        if let Some(route) = decision.routes.first() {
            let command_is_already_materialized = route
                .argv
                .iter()
                .filter(|word| !word.is_empty())
                .all(|word| decision.message.contains(word));
            if !command_is_already_materialized {
                let command = route
                    .argv
                    .iter()
                    .map(|word| shell_quote(word))
                    .collect::<Vec<_>>()
                    .join(" ");
                decision.message.push_str("\nASP route: `");
                decision.message.push_str(&command);
                decision.message.push('`');
            }
        }
        prepend_typed_agent_guidance(decision);
        return;
    }
    let reason = serde_json::to_value(decision.reason_kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "source-access".to_owned());
    decision.message = format!(
        "ASP denied source access (`{reason}`). Use `collaboration.spawn_agent` with the Config-resolved registered Agent, then run the parser-owned route in that agent."
    );
    prepend_typed_agent_guidance(decision);
}

fn prepend_typed_agent_guidance(decision: &mut HookDecision) {
    let string_field = |field: &str| {
        decision
            .fields
            .get(field)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
    };
    let Some(agent) = string_field("targetAgent").map(str::to_owned) else {
        return;
    };
    let receipt_kind = string_field("receiptKind")
        .unwrap_or("the configured role receipt")
        .to_owned();
    let symbol = string_field("targetAgentSymbol")
        .map(str::to_owned)
        .unwrap_or_else(|| format!("@{agent}"));
    let lane = string_field("executionLane")
        .map(str::to_owned)
        .unwrap_or_else(|| agent.clone());
    let jobs = match lane.as_str() {
        "testing" => "testing/build jobs",
        "explore" | "search" => "search/query jobs",
        _ => "this scoped job",
    };
    let guidance = format!(
        "Use `collaboration.spawn_agent` with `agent_type=\"{agent}\"` to create `{symbol}` for {jobs}; verify its canonical path with `collaboration.list_agents`, and require receipt `{receipt_kind}` before retrying."
    );
    decision.fields.insert(
        "targetAgentSymbol".to_owned(),
        serde_json::Value::String(symbol),
    );
    if !decision.message.contains(&guidance) {
        decision.message.insert(0, '\n');
        decision.message.insert_str(0, &guidance);
    }
}

fn shell_quote(word: &str) -> String {
    if !word.is_empty()
        && word
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_./:@%+=,-".contains(&byte))
    {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

pub(crate) fn with_executable_evidence_subagent_message(
    mut decision: HookDecision,
) -> HookDecision {
    if decision
        .message
        .contains("Return one compact `[asp-search-subagent]` graph-route receipt")
        && !decision
            .message
            .contains("Return executable `[asp-search-subagent]` evidence")
    {
        decision.message = decision.message.replace(
            "Return one compact `[asp-search-subagent]` graph-route receipt",
            "Return executable `[asp-search-subagent]` evidence with QueryGrammar, owner, item, selector, matchedBy, and relation. Return one compact `[asp-search-subagent]` graph-route receipt",
        );
    }
    decision
}
