use agent_semantic_hook::{DecisionKind, HookDecision};

pub(crate) fn materialize_source_access_deny_message(decision: &mut HookDecision) {
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
        "ASP denied source access (`{reason}`). Open the Org-owned interactive ChoicePlane with `asp session --agents choice-plane` to select a parser-owned recovery route."
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
    let Some(agent_name) = string_field("targetAgentName") else {
        return;
    };
    let agent_name = agent_name.trim_start_matches('@');
    let role = string_field("targetAgentRole").unwrap_or("configured");
    let description =
        string_field("targetAgentDescription").unwrap_or("the configured typed execution Agent");
    let lane = string_field("executionLane").unwrap_or(role);
    let jobs = match lane {
        "testing" => "testing/build jobs",
        "explore" | "search" => "search/query jobs",
        _ => "this scoped job",
    };
    let guidance = format!(
        "Please use `asp session --agents choice-plane` to create or resume `@{agent_name}` (role `{role}`: {description}) for {jobs}."
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

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_source_access_materialize.rs"]
mod tests;
