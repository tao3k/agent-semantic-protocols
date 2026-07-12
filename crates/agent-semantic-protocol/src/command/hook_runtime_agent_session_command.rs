//! Command token helpers for agent-session hook routing.

pub(super) fn command_requires_resident_child(
    command: &str,
    main_asp_command_allowed: impl Fn(&[String], usize) -> bool,
) -> bool {
    let tokens = shell_like_tokens(command);
    tokens.iter().enumerate().any(|(index, token)| {
        if !is_asp_binary_token(token) {
            return false;
        }
        match classify_main_session_asp_command(&tokens, index) {
            MainSessionAspCommandClass::ControlPlane
            | MainSessionAspCommandClass::ExactEvidenceRead => false,
            MainSessionAspCommandClass::ReasoningFlow => true,
            MainSessionAspCommandClass::Unknown => !main_asp_command_allowed(&tokens, index),
        }
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MainSessionAspCommandClass {
    ControlPlane,
    ExactEvidenceRead,
    ReasoningFlow,
    Unknown,
}

pub(super) fn classify_main_session_asp_command(
    tokens: &[String],
    asp_index: usize,
) -> MainSessionAspCommandClass {
    let Some(first) = tokens.get(asp_index + 1).map(|token| token.as_str()) else {
        return MainSessionAspCommandClass::Unknown;
    };
    if first.eq_ignore_ascii_case("agent") {
        return MainSessionAspCommandClass::ControlPlane;
    }
    if first.eq_ignore_ascii_case("search")
        || first.eq_ignore_ascii_case("query")
        || first.eq_ignore_ascii_case("rg")
        || first.eq_ignore_ascii_case("fd")
    {
        return MainSessionAspCommandClass::ReasoningFlow;
    }
    let Some(second) = tokens.get(asp_index + 2).map(|token| token.as_str()) else {
        return MainSessionAspCommandClass::Unknown;
    };
    if second.eq_ignore_ascii_case("search")
        || second.eq_ignore_ascii_case("rg")
        || second.eq_ignore_ascii_case("fd")
        || second.eq_ignore_ascii_case("elements-query")
    {
        return MainSessionAspCommandClass::ReasoningFlow;
    }
    if second.eq_ignore_ascii_case("contract")
        && matches!(
            tokens.get(asp_index + 3).map(String::as_str),
            Some("trace" | "query-surface")
        )
    {
        return MainSessionAspCommandClass::ReasoningFlow;
    }
    if !second.eq_ignore_ascii_case("query") {
        return MainSessionAspCommandClass::Unknown;
    }
    classify_query_command(tokens, asp_index + 3)
}

fn classify_query_command(
    tokens: &[String],
    query_args_start: usize,
) -> MainSessionAspCommandClass {
    if tokens
        .iter()
        .skip(query_args_start)
        .any(|token| token == "--term" || token == "-t")
    {
        return MainSessionAspCommandClass::ReasoningFlow;
    }
    let bounded_projection = tokens
        .iter()
        .skip(query_args_start)
        .any(|token| token == "--code" || token == "--names-only");
    let selector = selector_arg(tokens, query_args_start);
    match (selector, bounded_projection) {
        (Some(selector), true) if is_exact_parser_owned_item_selector(selector) => {
            MainSessionAspCommandClass::ExactEvidenceRead
        }
        _ => MainSessionAspCommandClass::ReasoningFlow,
    }
}

fn selector_arg(tokens: &[String], query_args_start: usize) -> Option<&str> {
    let mut index = query_args_start;
    while index < tokens.len() {
        let token = tokens[index].as_str();
        if token == "--selector" {
            return tokens.get(index + 1).map(String::as_str);
        }
        if token.starts_with('-') {
            index += if flag_takes_value(token) { 2 } else { 1 };
            continue;
        }
        return Some(token);
    }
    None
}

fn flag_takes_value(token: &str) -> bool {
    matches!(
        token,
        "--workspace" | "--view" | "--format" | "--limit" | "--lang" | "--language"
    )
}

fn is_exact_parser_owned_item_selector(selector: &str) -> bool {
    selector.contains("://") && selector.contains("#item/")
}

pub(super) fn command_contains_asp_binary(command: &str) -> bool {
    shell_like_tokens(command)
        .iter()
        .any(|token| is_asp_binary_token(token))
}

pub(super) fn command_prefix_tokens(prefix: &str) -> Result<Vec<String>, String> {
    let tokens = prefix
        .split_whitespace()
        .map(|token| token.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        Err("aspSessionPolicy.mainAllowedAspCommandPrefixes[] must not be empty".to_string())
    } else {
        Ok(tokens)
    }
}

pub(super) fn command_prefix_matches(
    tokens: &[String],
    asp_index: usize,
    prefix: &[String],
) -> bool {
    let command_start = asp_index + 1;
    if tokens.len() <= command_start {
        return prefix.len() == 1 && prefix[0] == "help";
    }
    tokens.len() >= command_start + prefix.len()
        && tokens
            .iter()
            .skip(command_start)
            .zip(prefix.iter())
            .all(|(token, expected)| token.eq_ignore_ascii_case(expected))
}

pub(super) fn command_prefix_matches_wrapped(tokens: &[String], prefix: &[String]) -> bool {
    let command_start = command_start_after_wrappers(tokens);
    tokens.len() >= command_start + prefix.len()
        && tokens
            .iter()
            .skip(command_start)
            .zip(prefix.iter())
            .all(|(token, expected)| token.eq_ignore_ascii_case(expected))
}

fn command_start_after_wrappers(tokens: &[String]) -> usize {
    let mut index = 0;
    if tokens
        .get(index)
        .is_some_and(|token| token.eq_ignore_ascii_case("direnv"))
        && tokens
            .get(index + 1)
            .is_some_and(|token| token.eq_ignore_ascii_case("exec"))
    {
        index += 2;
        if index < tokens.len() {
            index += 1;
        }
    }
    if tokens
        .get(index)
        .is_some_and(|token| is_env_command_token(token))
    {
        index += 1;
        while tokens
            .get(index)
            .is_some_and(|token| is_env_assignment_token(token))
        {
            index += 1;
        }
    }
    index
}

fn is_env_command_token(token: &str) -> bool {
    token.rsplit('/').next() == Some("env")
}

fn is_env_assignment_token(token: &str) -> bool {
    let Some((name, _value)) = token.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric())
}

pub(super) fn is_asp_binary_token(token: &str) -> bool {
    token.rsplit('/').next() == Some("asp")
}

pub(super) fn shell_like_tokens(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .map(|token| {
            token.trim_matches(|character: char| {
                matches!(
                    character,
                    '\'' | '"' | ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
                )
            })
        })
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}
