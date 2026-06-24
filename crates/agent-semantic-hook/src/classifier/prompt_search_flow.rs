//! Prompt-local `search prime`/`search pipe` feedback for hook classification.

use serde_json::Value;

use crate::event_state::{
    AspDirectSourceReadShape, AspSearchCommandStage, asp_command_tokens,
    asp_query_code_or_direct_read_tokens, asp_query_direct_source_read_shape_tokens,
    asp_search_stage_tokens, prompt_asp_command_count, prompt_search_flow_after_prime,
};
use crate::{
    DecisionKind, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, HookDecision, HookRuntime, OperationIntent, ReasonKind, ToolAction,
    payload_string, subject_for_action,
};

const LOW_PRIORITY_DIRECT_SOURCE_READ_MAX_LINES: usize = 80;

pub(super) fn classify_prompt_search_flow_feedback(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    payload: &Value,
    action: &ToolAction,
) -> Option<HookDecision> {
    if event != "pre-tool" {
        return None;
    }
    if !action_supports_prompt_search_flow_feedback(action) {
        return None;
    }
    let command_tokens = action.command_tokens()?;
    let search_stage = asp_search_stage_tokens(&command_tokens);
    let query_code_or_direct_read = asp_query_code_or_direct_read_tokens(&command_tokens);
    let direct_source_read_shape = asp_query_direct_source_read_shape_tokens(&command_tokens);
    let asp_budget_enabled = prompt_asp_command_budget().is_some();
    if search_stage.is_none()
        && !query_code_or_direct_read
        && direct_source_read_shape.is_none()
        && !(asp_budget_enabled && asp_command_tokens(&command_tokens))
    {
        return None;
    }
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
    .flatten()?;
    if !feedback.saw_pipe {
        match search_stage {
            Some(AspSearchCommandStage::Prime(language_id))
                if language_id == feedback.language_id =>
            {
                return Some(search_flow_feedback_decision(
                    platform,
                    event,
                    action,
                    &feedback.language_id,
                    "repeat-prime-before-pipe",
                    "ASP hook denied repeated `search prime` before `search pipe`.",
                ));
            }
            _ => {}
        }
        if query_code_or_direct_read {
            return Some(search_flow_feedback_decision(
                platform,
                event,
                action,
                &feedback.language_id,
                "read-before-pipe",
                "ASP hook denied code/direct read before `search pipe`.",
            ));
        }
        return None;
    }
    match search_stage {
        Some(AspSearchCommandStage::Pipe(language_id)) if language_id == feedback.language_id => {
            return Some(search_flow_feedback_decision(
                platform,
                event,
                action,
                &feedback.language_id,
                "repeat-search-pipe",
                "ASP hook denied repeated `search pipe` in the same prompt.",
            ));
        }
        _ => {}
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
                    &feedback.language_id,
                    "direct-source-read-after-pipe",
                    "ASP hook denied broad manual `direct-source-read` after `search pipe`.",
                ));
            }
        }
    }
    if let Some(decision) = classify_prompt_asp_command_budget(
        registry,
        platform,
        event,
        action,
        asp_command_tokens(&command_tokens),
        session_id.as_deref(),
        transcript_path.as_deref(),
    ) {
        return Some(decision);
    }
    None
}

fn action_supports_prompt_search_flow_feedback(action: &ToolAction) -> bool {
    matches!(
        action.operation,
        OperationIntent::ShellCommand | OperationIntent::StdinContinuation
    )
}

fn classify_prompt_asp_command_budget(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    is_asp_command: bool,
    session_id: Option<&str>,
    transcript_path: Option<&str>,
) -> Option<HookDecision> {
    let max_commands = prompt_asp_command_budget()?;
    if !is_asp_command {
        return None;
    }
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
                "Use `asp {language_id} search owner <owner-path> items --query '<symbol-or-a|b|c>' --workspace . --view seeds`, `asp fd -query '<owner-or-path-term-a|term-b|term-c>' <scope>`, `asp rg -query '<content-or-error-term-a|term-b|term-c>' <scope>`, or `asp {language_id} query --selector <path:start-end> --workspace <workspace-root> {projection_flag}`."
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
            "`--from-hook direct-source-read` is a low-priority exact-window fallback, not the first code extraction path."
                .to_string(),
            String::new(),
            "## Run Next".to_string(),
            format!(
                "asp {language_id} query --selector <path:start-end> --workspace <workspace-root> {projection_flag}"
            ),
            String::new(),
            "## Rules".to_string(),
            format!("A direct-source-read fallback is allowed only after `search pipe` when the selector is an exact bounded range of {LOW_PRIORITY_DIRECT_SOURCE_READ_MAX_LINES} lines or fewer."),
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
            format!(
                "asp {language_id} search pipe '<question-or-feature-term>' --workspace . --view seeds"
            ),
            String::new(),
            "## Rules".to_string(),
            "Compress the user's question into one code-search seed before running the pipe."
                .to_string(),
            "Do not repeat `search prime`. Do not read source or code before the pipe frontier."
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
