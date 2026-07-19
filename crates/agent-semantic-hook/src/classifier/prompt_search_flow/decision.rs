use super::{feedback, guidance};
use crate::classifier::decision::allow;
use crate::tool_action::{ToolAction, subject_for_action};
use agent_semantic_config::AspCommandIntent;

use crate::{
    AspLanguageCommand, DecisionKind, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, ReasonKind,
};
use serde_json::Value;

pub(super) fn search_flow_feedback_decision(
    platform: &str,
    event: &str,
    action: &ToolAction,
    language_id: &str,
    outcome: feedback::PromptSearchFeedback,
) -> HookDecision {
    let (command_intent, feedback_kind, heading) = match outcome {
        feedback::PromptSearchFeedback::RepeatPrimeBeforePipe => (
            AspCommandIntent::Reasoning,
            "repeat-prime-before-pipe",
            "ASP hook denied repeated `search prime` before `search pipe`.",
        ),
        feedback::PromptSearchFeedback::ReadBeforePipe => (
            AspCommandIntent::DirectReadFallback,
            "read-before-pipe",
            "ASP hook denied code/direct read before `search pipe`.",
        ),
        feedback::PromptSearchFeedback::RepeatSearchPipe => (
            AspCommandIntent::Reasoning,
            "repeat-search-pipe",
            "ASP hook denied exact replay of `search pipe` in the same prompt.",
        ),
        feedback::PromptSearchFeedback::DirectSourceReadAfterPipe => (
            AspCommandIntent::DirectReadFallback,
            "direct-source-read-after-pipe",
            "ASP hook denied broad manual `direct-source-read` after `search pipe`.",
        ),
    };
    let mut fields = std::collections::BTreeMap::new();
    fields.insert(
        "hookFeedback".to_string(),
        Value::String(feedback_kind.to_string()),
    );
    fields.insert(
        "languageId".to_string(),
        Value::String(language_id.to_string()),
    );
    fields.insert(
        "aspCommandIntent".to_string(),
        Value::String(command_intent.as_str().to_string()),
    );
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::None,
        language_ids: vec![language_id.to_string()],
        subject: subject_for_action(action),
        routes: Vec::new(),
        message: guidance::search_flow_feedback_message(
            language_id,
            feedback_kind,
            heading,
            guidance::query_projection_flag(language_id),
        ),
        fields,
    }
}

pub(super) fn exhausted_asp_command_budget_decision(
    platform: &str,
    event: &str,
    action: &ToolAction,
    budget: &super::budget::ExhaustedAspCommandBudget,
) -> HookDecision {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert(
        "hookFeedback".to_string(),
        Value::String("asp-command-budget-exhausted".to_string()),
    );
    fields.insert(
        "aspCommandCount".to_string(),
        Value::Number(serde_json::Number::from(budget.command_count)),
    );
    fields.insert(
        "maxAspCommands".to_string(),
        Value::Number(serde_json::Number::from(budget.max_commands)),
    );
    fields.insert(
        "aspCommandIntent".to_string(),
        Value::String(AspCommandIntent::Reasoning.as_str().to_string()),
    );
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::None,
        language_ids: Vec::new(),
        subject: subject_for_action(action),
        routes: Vec::new(),
        message: super::budget::asp_command_budget_message(budget),
        fields,
    }
}

pub(super) fn allow_asp_language_command(
    platform: &str,
    event: &str,
    action: &ToolAction,
    command: &AspLanguageCommand,
) -> HookDecision {
    let mut decision = allow(platform, event, subject_for_action(action));
    decision.language_ids = vec![command.language_id.clone()];
    decision.fields.insert(
        "languageId".to_string(),
        Value::String(command.language_id.clone()),
    );
    decision.fields.insert(
        "aspCommandIntent".to_string(),
        Value::String(command.intent.as_str().to_string()),
    );
    decision.fields.insert(
        "aspCommandRoute".to_string(),
        Value::String(command.route.wire_value()),
    );
    if let Some(selector) = command.selector.as_ref() {
        decision
            .fields
            .insert("selector".to_string(), Value::String(selector.clone()));
    }
    decision
}

pub(super) fn invalid_evidence_query_decision(
    platform: &str,
    event: &str,
    action: &ToolAction,
    command: &AspLanguageCommand,
) -> HookDecision {
    let selector = command.selector.as_deref().unwrap_or("<missing-selector>");
    let mut fields = std::collections::BTreeMap::new();
    fields.insert(
        "hookFeedback".to_string(),
        Value::String("invalid-evidence-query-denied".to_string()),
    );
    fields.insert(
        "languageId".to_string(),
        Value::String(command.language_id.clone()),
    );
    fields.insert(
        "aspCommandIntent".to_string(),
        Value::String(command.intent.as_str().to_string()),
    );
    fields.insert(
        "aspCommandRoute".to_string(),
        Value::String(command.route.wire_value()),
    );
    fields.insert("selector".to_string(), Value::String(selector.to_string()));
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::DirectSourceRead,
        language_ids: vec![command.language_id.clone()],
        subject: subject_for_action(action),
        routes: Vec::new(),
        message: guidance::invalid_evidence_query_message(&command.language_id, selector),
        fields,
    }
}
