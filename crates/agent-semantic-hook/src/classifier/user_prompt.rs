//! Classifies user-prompt scope without coupling it to tool-action policy owners.

use serde_json::Value;

use super::decision::allow;
use crate::DecisionSubject;
use crate::HookDecision;
use crate::payload_string;

pub(crate) fn classify_user_prompt(
    platform: &str,
    event: &str,
    payload: &Value,
) -> Option<HookDecision> {
    if event != "user-prompt" {
        return None;
    }
    let prompt = payload_string(payload, "prompt").unwrap_or_default();
    let mut decision = allow(platform, event, DecisionSubject::default());
    if prompt_is_locator_only(&prompt) {
        decision.fields.insert(
            "promptWorkflow".to_string(),
            Value::String("locator-only".to_string()),
        );
    }
    Some(decision)
}

pub(super) fn prompt_is_locator_only(prompt: &str) -> bool {
    let prompt = prompt.to_ascii_lowercase();
    (prompt.contains("where ")
        || prompt.contains("locate")
        || prompt.contains("located")
        || prompt.contains("selecting files")
        || prompt.contains("before selecting"))
        && !prompt.contains("show code")
        && !prompt.contains("read code")
        && !prompt.contains("extract code")
}
