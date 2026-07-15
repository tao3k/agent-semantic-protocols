//! Command token helpers for agent-session hook routing.

use agent_semantic_config::HookClientAspCommandIntentPolicyConfig;
use agent_semantic_hook::{
    AspLanguageCommandIntent, asp_invocation_indices,
    classify_asp_language_command_tokens_with_policy, semantic_shell_tokens,
};

pub(super) fn command_requires_resident_child(
    command: &str,
    intent_policy: &HookClientAspCommandIntentPolicyConfig,
) -> bool {
    let tokens = semantic_shell_tokens(command);
    asp_invocation_indices(&tokens).into_iter().any(
        |index| match classify_main_session_asp_command(&tokens, index, intent_policy) {
            MainSessionAspCommandClass::ControlPlane
            | MainSessionAspCommandClass::ExactEvidenceRead
            | MainSessionAspCommandClass::DirectReadFallback
            | MainSessionAspCommandClass::InvalidEvidence => false,
            MainSessionAspCommandClass::ReasoningFlow => true,
            MainSessionAspCommandClass::Unknown => false,
        },
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MainSessionAspCommandClass {
    ControlPlane,
    ExactEvidenceRead,
    DirectReadFallback,
    InvalidEvidence,
    ReasoningFlow,
    Unknown,
}

pub(super) fn classify_main_session_asp_command(
    tokens: &[String],
    asp_index: usize,
    intent_policy: &HookClientAspCommandIntentPolicyConfig,
) -> MainSessionAspCommandClass {
    let Some(first) = tokens.get(asp_index + 1).map(|token| token.as_str()) else {
        return MainSessionAspCommandClass::Unknown;
    };
    if let Some(command) =
        classify_asp_language_command_tokens_with_policy(&tokens[asp_index..], intent_policy)
    {
        return match command.intent {
            AspLanguageCommandIntent::ExactEvidence => {
                MainSessionAspCommandClass::ExactEvidenceRead
            }
            AspLanguageCommandIntent::Reasoning => MainSessionAspCommandClass::ReasoningFlow,
            AspLanguageCommandIntent::DirectReadFallback => {
                MainSessionAspCommandClass::DirectReadFallback
            }
            AspLanguageCommandIntent::InvalidEvidence => {
                MainSessionAspCommandClass::InvalidEvidence
            }
        };
    }
    if intent_policy
        .control_plane
        .root_commands
        .iter()
        .any(|command| first.eq_ignore_ascii_case(command))
    {
        return MainSessionAspCommandClass::ControlPlane;
    }
    if intent_policy
        .reasoning
        .root_commands
        .iter()
        .any(|command| first.eq_ignore_ascii_case(command))
    {
        return MainSessionAspCommandClass::ReasoningFlow;
    }
    MainSessionAspCommandClass::Unknown
}

pub(super) fn command_contains_asp_binary(command: &str) -> bool {
    semantic_shell_tokens(command)
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
    token == "asp"
        || token.rsplit('/').next() == Some("asp")
        || token.rsplit('\\').next() == Some("asp.exe")
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
