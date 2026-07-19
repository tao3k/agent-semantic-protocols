//! Prompt-local `search prime`/`search pipe` feedback for hook classification.

use serde_json::Value;

use agent_semantic_config::AspCommandIntent;

use crate::command::{AspLanguageCommand, classify_asp_language_command_tokens_with_policy};
use crate::event_state::{
    AspDirectSourceReadShape, asp_query_direct_source_read_shape_tokens, prompt_asp_command_count,
    prompt_search_flow_after_prime,
};
use crate::{HookDecision, HookRuntime, OperationIntent, ToolAction, payload_string};

use super::{budget, decision, feedback};

pub(crate) fn materialize_prompt_search_strategy_decision(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    payload: &Value,
    action: &ToolAction,
    intent_policy: &agent_semantic_config::HookClientAspCommandIntentPolicyConfig,
) -> Option<HookDecision> {
    if event != "pre-tool" {
        return None;
    }
    if !action_supports_prompt_search_flow_feedback(action) {
        return None;
    }
    let command_tokens = action.command_tokens()?;
    if asp_agent_session_command_tokens(&command_tokens) {
        return None;
    }
    let asp_command =
        classify_asp_language_command_tokens_with_policy(&command_tokens, intent_policy)?;
    if !registry
        .providers
        .iter()
        .any(|provider| provider.language_id == asp_command.language_id)
    {
        return None;
    }
    if asp_command.intent == AspCommandIntent::InvalidEvidence {
        return Some(decision::invalid_evidence_query_decision(
            platform,
            event,
            action,
            &asp_command,
        ));
    }
    if asp_command.intent == AspCommandIntent::ExactEvidence {
        return Some(decision::allow_asp_language_command(
            platform,
            event,
            action,
            &asp_command,
        ));
    }
    let direct_source_read_shape = asp_query_direct_source_read_shape_tokens(&command_tokens);
    let session_id =
        payload_string(payload, "session_id").or_else(|| payload_string(payload, "sessionId"));
    let transcript_path = payload_string(payload, "transcript_path")
        .or_else(|| payload_string(payload, "transcriptPath"));
    let feedback = prompt_search_flow_after_prime(
        std::path::Path::new(&registry.project_root),
        session_id.as_deref(),
        transcript_path.as_deref(),
    )
    .ok()
    .flatten();
    if let Some(feedback) = feedback.as_ref()
        && let Some(decision) = classify_prompt_search_flow_feedback(
            platform,
            event,
            action,
            &asp_command,
            command_tokens.as_ref(),
            direct_source_read_shape,
            &feedback.language_id,
            feedback.saw_pipe,
            &feedback.pipe_command_tokens,
        )
    {
        return Some(decision);
    }
    if asp_command.intent == AspCommandIntent::Reasoning
        && let Some(decision) = classify_prompt_asp_command_budget(
            registry,
            platform,
            event,
            action,
            session_id.as_deref(),
            transcript_path.as_deref(),
        )
    {
        return Some(decision);
    }
    Some(decision::allow_asp_language_command(
        platform,
        event,
        action,
        &asp_command,
    ))
}

fn classify_prompt_search_flow_feedback(
    platform: &str,
    event: &str,
    action: &ToolAction,
    asp_command: &AspLanguageCommand,
    command_tokens: &[String],
    direct_source_read_shape: Option<AspDirectSourceReadShape>,
    feedback_language_id: &str,
    saw_pipe: bool,
    pipe_command_tokens: &[Vec<String>],
) -> Option<HookDecision> {
    let direct_source_read = match direct_source_read_shape {
        None => feedback::DirectSourceReadScope::None,
        Some(AspDirectSourceReadShape::Bounded { line_span }) => {
            feedback::DirectSourceReadScope::Bounded(line_span)
        }
        Some(_) => feedback::DirectSourceReadScope::Broad,
    };
    let outcome =
        feedback::evaluate_prompt_search_feedback(feedback::PromptSearchFeedbackRequest {
            route: &asp_command.route,
            intent: asp_command.intent,
            same_language: asp_command.language_id == feedback_language_id,
            saw_pipe,
            repeated_pipe: pipe_command_tokens
                .iter()
                .any(|previous| previous.as_slice() == command_tokens),
            direct_source_read,
            bounded_read_max_lines: feedback::LOW_PRIORITY_DIRECT_SOURCE_READ_MAX_LINES,
        })?;
    Some(decision::search_flow_feedback_decision(
        platform,
        event,
        action,
        feedback_language_id,
        outcome,
    ))
}

fn action_supports_prompt_search_flow_feedback(action: &ToolAction) -> bool {
    matches!(
        action.operation,
        OperationIntent::ShellCommand | OperationIntent::StdinContinuation
    )
}

fn asp_agent_session_command_tokens(command_tokens: &[String]) -> bool {
    command_tokens.windows(3).any(|tokens| {
        is_asp_command_token(&tokens[0]) && tokens[1] == "agent" && tokens[2] == "session"
    })
}

fn is_asp_command_token(token: &str) -> bool {
    token == "asp" || token.ends_with("/asp") || token.ends_with("\\asp.exe")
}

fn classify_prompt_asp_command_budget(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    session_id: Option<&str>,
    transcript_path: Option<&str>,
) -> Option<HookDecision> {
    let command_count = prompt_asp_command_count(
        std::path::Path::new(&registry.project_root),
        session_id,
        transcript_path,
    )
    .ok()?;
    let budget = budget::exhausted_asp_command_budget(command_count)?;
    Some(decision::exhausted_asp_command_budget_decision(
        platform, event, action, &budget,
    ))
}
