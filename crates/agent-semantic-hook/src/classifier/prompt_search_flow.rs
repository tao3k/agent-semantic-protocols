//! Prompt-local `search prime`/`search pipe` feedback for hook classification.

use serde_json::Value;

use super::decision::allow;
use crate::command::{
    AspLanguageCommand, AspLanguageCommandIntent, classify_asp_language_command_tokens_with_policy,
};
use crate::event_state::{
    AspDirectSourceReadShape, asp_query_direct_source_read_shape_tokens, prompt_asp_command_count,
    prompt_search_flow_after_prime,
};
use crate::{
    DecisionKind, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, HookDecision, HookRuntime, OperationIntent, ReasonKind, ToolAction,
    payload_string, subject_for_action,
};

const LOW_PRIORITY_DIRECT_SOURCE_READ_MAX_LINES: usize = 80;

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
    if asp_command.intent == AspLanguageCommandIntent::InvalidEvidence {
        return Some(invalid_evidence_query_decision(
            platform,
            event,
            action,
            &asp_command,
        ));
    }
    if asp_command.intent == AspLanguageCommandIntent::ExactEvidence {
        return Some(allow_asp_language_command(
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
    if asp_command.intent == AspLanguageCommandIntent::Reasoning
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
    Some(allow_asp_language_command(
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
    if !saw_pipe {
        if asp_command.route == "search-prime" && asp_command.language_id == feedback_language_id {
            return Some(search_flow_feedback_decision(
                platform,
                event,
                action,
                feedback_language_id,
                AspLanguageCommandIntent::Reasoning,
                "repeat-prime-before-pipe",
                "ASP hook denied repeated `search prime` before `search pipe`.",
            ));
        }
        if asp_command.intent == AspLanguageCommandIntent::DirectReadFallback {
            return Some(search_flow_feedback_decision(
                platform,
                event,
                action,
                feedback_language_id,
                AspLanguageCommandIntent::DirectReadFallback,
                "read-before-pipe",
                "ASP hook denied code/direct read before `search pipe`.",
            ));
        }
        return None;
    }

    if asp_command.route == "search-pipe"
        && asp_command.language_id == feedback_language_id
        && pipe_command_tokens
            .iter()
            .any(|previous| previous.as_slice() == command_tokens)
    {
        return Some(search_flow_feedback_decision(
            platform,
            event,
            action,
            feedback_language_id,
            AspLanguageCommandIntent::Reasoning,
            "repeat-search-pipe",
            "ASP hook denied exact replay of `search pipe` in the same prompt.",
        ));
    }
    if let Some(shape) = direct_source_read_shape {
        match shape {
            AspDirectSourceReadShape::Bounded { line_span }
                if line_span <= LOW_PRIORITY_DIRECT_SOURCE_READ_MAX_LINES => {}
            _ => {
                return Some(search_flow_feedback_decision(
                    platform,
                    event,
                    action,
                    feedback_language_id,
                    AspLanguageCommandIntent::DirectReadFallback,
                    "direct-source-read-after-pipe",
                    "ASP hook denied broad manual `direct-source-read` after `search pipe`.",
                ));
            }
        }
    }
    None
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
    let max_commands = prompt_asp_command_budget()?;
    let command_count = prompt_asp_command_count(
        std::path::Path::new(&registry.project_root),
        session_id,
        transcript_path,
    )
    .ok()?;
    if command_count < max_commands {
        return None;
    }
    let mut fields = std::collections::BTreeMap::new();
    fields.insert(
        "hookFeedback".to_string(),
        Value::String("asp-command-budget-exhausted".to_string()),
    );
    fields.insert(
        "aspCommandCount".to_string(),
        Value::Number(serde_json::Number::from(command_count)),
    );
    fields.insert(
        "maxAspCommands".to_string(),
        Value::Number(serde_json::Number::from(max_commands)),
    );
    fields.insert(
        "aspCommandIntent".to_string(),
        Value::String(AspLanguageCommandIntent::Reasoning.as_str().to_string()),
    );
    Some(HookDecision {
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
        message: asp_command_budget_message(command_count, max_commands),
        fields,
    })
}

fn prompt_asp_command_budget() -> Option<usize> {
    std::env::var("ASP_HOOK_MAX_ASP_COMMANDS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
}

fn asp_command_budget_message(command_count: usize, max_commands: usize) -> String {
    [
        format!(
            "ASP hook denied ASP command budget exhaustion: {command_count}/{max_commands} commands already completed."
        ),
        "Answer from the existing ASP frontier, recommendedNext, nextCommand, owner, locator, and output metadata instead of running more commands.".to_string(),
        String::new(),
        "## Rules".to_string(),
        "Do not run more ASP commands in this prompt after the budget is exhausted.".to_string(),
        "Do not switch to raw shell reads; use the evidence already returned by ASP.".to_string(),
    ]
    .join("\n")
}

fn search_flow_feedback_decision(
    platform: &str,
    event: &str,
    action: &ToolAction,
    language_id: &str,
    command_intent: AspLanguageCommandIntent,
    feedback_kind: &str,
    heading: &str,
) -> HookDecision {
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
        message: search_flow_feedback_message(language_id, feedback_kind, heading),
        fields,
    }
}

fn allow_asp_language_command(
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
        Value::String(command.route.clone()),
    );
    if let Some(selector) = command.selector.as_ref() {
        decision
            .fields
            .insert("selector".to_string(), Value::String(selector.clone()));
    }
    decision
}

fn invalid_evidence_query_decision(
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
        Value::String(command.route.clone()),
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
        message: invalid_evidence_query_message(&command.language_id, selector),
        fields,
    }
}

fn invalid_evidence_query_message(language_id: &str, selector: &str) -> String {
    [
        format!("ASP hook denied non-exact evidence query selector `{selector}`."),
        "Evidence projection requires one parser-owned structural item selector.".to_string(),
        String::new(),
        "## Run Next".to_string(),
        format!(
            "Ask `asp-explore` to run `asp {language_id} search owner <owner-path> items --query '<symbol-or-a|b|c>' --workspace . --view seeds` and return selector-only `[asp-search-subagent]` receipts."
        ),
        String::new(),
        "## Rules".to_string(),
        "The parent exact read must use a parser-owned item selector such as `rust://...#item/function/name`."
            .to_string(),
        "Do not use file-level `--code`, line-range selectors, or raw source reads as search evidence."
            .to_string(),
    ]
    .join("\n")
}

fn search_flow_feedback_message(language_id: &str, feedback_kind: &str, heading: &str) -> String {
    let projection_flag = query_projection_flag(language_id);
    match feedback_kind {
        "repeat-search-pipe" => [
            heading.to_string(),
            "The current prompt has already run `search pipe`; pipe is a once-per-prompt frontier."
                .to_string(),
            String::new(),
            "## Run Next".to_string(),
            "Follow the previous `recommendedNext` / `nextCommand` from the pipe output."
                .to_string(),
            format!(
                "Use `asp {language_id} search owner <owner-path> items --query '<symbol-or-a|b|c>' --workspace . --view seeds`, `asp fd -query '<owner-or-path-term-a|term-b|term-c>' <scope>`, `asp rg -query '<content-or-error-term-a|term-b|term-c>' <scope>`, or `asp {language_id} query --selector '<language>://<owner>#item/<kind>/<name>' --workspace <workspace-root> {projection_flag}`."
            ),
            String::new(),
            "## Rules".to_string(),
            "Do not rerun `search pipe` with a narrower natural term in the same prompt."
                .to_string(),
            format!(
                "Move from frontier to locator/action; keep source reads behind exact `query --selector {projection_flag}`."
            ),
        ]
        .join("\n"),
        "direct-source-read-after-pipe" => [
            heading.to_string(),
            "`--from-hook direct-source-read` is a low-priority exact-window fallback after a route frontier, not the first code extraction path."
                .to_string(),
            String::new(),
            "## Run Next".to_string(),
            format!(
                "asp {language_id} query --from-hook direct-source-read --selector <path:start-end> --workspace <workspace-root> {projection_flag}"
            ),
            String::new(),
            "## Rules".to_string(),
            format!("A direct-source-read fallback is allowed only after owner/query/syntax/read-plan evidence when the selector is an exact bounded range of {LOW_PRIORITY_DIRECT_SOURCE_READ_MAX_LINES} lines or fewer."),
            format!(
                "For file-wide or broad ranges, follow locator/frontier commands and extract with ordinary `query --selector {projection_flag}` or a read-plan frontier."
            ),
        ]
        .join("\n"),
        _ => [
            heading.to_string(),
            "The current prompt has already run `search prime`; prime is only a project map."
                .to_string(),
            String::new(),
            "## Run Next".to_string(),
            "Choose the next ASP route from the current evidence state.".to_string(),
            String::new(),
            "## Rules".to_string(),
            "Follow `recommendedNext` or `nextCommand` when the prime packet supplied one."
                .to_string(),
            format!(
                "Run `asp {language_id} search pipe '<question-or-feature-term>' --workspace . --view seeds` only when the evidence is still ambiguous and needs query refinement."
            ),
            "If an owner, symbol, dependency, test/failure, or exact selector is already known, skip pipe and use the narrower owner/reasoning/query route."
                .to_string(),
            "Do not repeat `search prime`. Do not read source or code before exact parser-owned identity or a route frontier justifies it."
                .to_string(),
        ]
        .join("\n"),
    }
}

fn query_projection_flag(language_id: &str) -> &'static str {
    if matches!(language_id, "md" | "org") {
        "--content"
    } else {
        "--code"
    }
}
