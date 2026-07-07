//! Prompt-local `search prime`/`search pipe` feedback for hook classification.

use serde_json::Value;

use crate::event_state::{
    AspDirectSourceReadShape, AspSearchCommandStage, asp_command_tokens,
    asp_query_code_or_direct_read_tokens, asp_query_direct_inventory_or_fetch_tokens,
    asp_query_direct_source_read_shape_tokens, asp_search_stage_tokens, prompt_asp_command_count,
    prompt_search_flow_after_prime,
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
    let parser_owned_query_code = asp_parser_owned_query_code_tokens(&command_tokens);
    let direct_inventory_or_fetch = asp_query_direct_inventory_or_fetch_tokens(&command_tokens);
    let direct_source_read_shape = asp_query_direct_source_read_shape_tokens(&command_tokens);
    if let Some((language_id, selector)) = file_level_query_code_selector(registry, &command_tokens)
    {
        return Some(file_level_query_code_decision(
            platform,
            event,
            action,
            &language_id,
            &selector,
        ));
    }
    let agent_session_lifecycle_command =
        search_stage.is_none() && asp_agent_session_command_tokens(&command_tokens);
    if agent_session_lifecycle_command {
        return None;
    }
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
        if query_code_or_direct_read && !direct_inventory_or_fetch && !parser_owned_query_code {
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

fn asp_parser_owned_query_code_tokens(tokens: &[String]) -> bool {
    let Some(asp_index) = tokens.iter().position(|token| is_asp_command_token(token)) else {
        return false;
    };
    let after_asp = &tokens[asp_index + 1..];
    let query_tokens = if after_asp.first().map(String::as_str) == Some("query") {
        after_asp
    } else if after_asp.get(1).map(String::as_str) == Some("query") {
        &after_asp[1..]
    } else {
        return false;
    };
    query_tokens.iter().any(|token| token == "--code")
        && option_value(query_tokens, "--selector").is_some_and(selector_is_parser_owned)
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

fn file_level_query_code_selector(
    registry: &HookRuntime,
    tokens: &[String],
) -> Option<(String, String)> {
    let asp_index = tokens
        .iter()
        .position(|token| is_asp_command_token(token))?;
    let after_asp = &tokens[asp_index + 1..];
    let (language_id, query_tokens) = if after_asp.first().map(String::as_str) == Some("query") {
        (language_from_flags(after_asp)?, after_asp)
    } else if after_asp.get(1).map(String::as_str) == Some("query") {
        (after_asp.first()?.clone(), &after_asp[1..])
    } else {
        return None;
    };
    if !query_tokens.iter().any(|token| token == "--code") {
        return None;
    }
    if matches!(
        option_value(query_tokens, "--from-hook"),
        Some("direct-source-read")
    ) {
        return None;
    }
    let selector = option_value(query_tokens, "--selector")?;
    if selector_is_parser_owned(selector)
        || !selector_is_source_file_or_line_range(registry, selector)
    {
        return None;
    }
    Some((language_id, selector.to_string()))
}

fn language_from_flags(tokens: &[String]) -> Option<String> {
    option_value(tokens, "--language").map(str::to_string)
}

fn option_value<'a>(tokens: &'a [String], option: &str) -> Option<&'a str> {
    tokens.windows(2).find_map(|pair| {
        if pair[0] == option {
            Some(pair[1].as_str())
        } else {
            None
        }
    })
}

fn selector_is_parser_owned(selector: &str) -> bool {
    selector.contains("://") && selector.contains("#item/")
}

fn selector_is_source_file_or_line_range(registry: &HookRuntime, selector: &str) -> bool {
    let path = selector_before_line_range(selector);
    let Some(extension) = std::path::Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
    else {
        return false;
    };
    registry.providers.iter().any(|provider| {
        provider
            .source_extensions
            .iter()
            .any(|source| source.trim_start_matches('.') == extension)
    })
}

fn selector_before_line_range(selector: &str) -> &str {
    let mut path = selector;
    for _ in 0..2 {
        let Some((candidate, suffix)) = path.rsplit_once(':') else {
            break;
        };
        if !is_line_range(suffix) {
            break;
        }
        path = candidate;
    }
    path
}

fn is_line_range(value: &str) -> bool {
    if let Some((start, end)) = value.split_once('-') {
        is_decimal(start) && is_decimal(end)
    } else {
        is_decimal(value)
    }
}

fn is_decimal(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_asp_command_token(token: &str) -> bool {
    token == "asp" || token.ends_with("/asp") || token.ends_with("\\asp.exe")
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

fn file_level_query_code_decision(
    platform: &str,
    event: &str,
    action: &ToolAction,
    language_id: &str,
    selector: &str,
) -> HookDecision {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert(
        "hookFeedback".to_string(),
        Value::String("file-level-query-code-denied".to_string()),
    );
    fields.insert(
        "languageId".to_string(),
        Value::String(language_id.to_string()),
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
        language_ids: vec![language_id.to_string()],
        subject: subject_for_action(action),
        routes: Vec::new(),
        message: file_level_query_code_message(language_id, selector),
        fields,
    }
}

fn file_level_query_code_message(language_id: &str, selector: &str) -> String {
    [
        format!("ASP hook denied file-level `query --code` for `{selector}`."),
        "A source file path is not a parser-owned exact read selector.".to_string(),
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
            "`--from-hook direct-source-read` is a low-priority exact-window fallback after a route frontier, not the first code extraction path."
                .to_string(),
            String::new(),
            "## Run Next".to_string(),
            format!(
                "asp {language_id} query --selector <path:start-end> --workspace <workspace-root> {projection_flag}"
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
