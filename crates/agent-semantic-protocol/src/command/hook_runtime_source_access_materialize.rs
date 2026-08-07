use agent_semantic_hook::{DecisionKind, HookDecision};

pub(crate) fn materialize_source_access_deny_message(decision: &mut HookDecision) {
    if decision.decision != DecisionKind::Deny {
        return;
    }
    if !decision.message.trim().is_empty() {
        if let Some(language_id) = decision.language_ids.first() {
            decision.message = decision
                .message
                .replace("{{languageId}}", language_id.as_str());
        }
        if let Some(route) = decision.routes.first() {
            let command = route
                .argv
                .iter()
                .map(|word| shell_quote(word))
                .collect::<Vec<_>>()
                .join(" ");
            if !command.is_empty() && !decision.message.contains(&command) {
                decision.message.push_str("\nASP route: `");
                decision.message.push_str(&command);
                decision.message.push('`');
            }
        }
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
