//! Parser-owned classification for public `asp <language>` commands.

use agent_semantic_config::HookClientAspCommandIntentPolicyConfig;

/// Parser-owned intent of one public `asp <language>` command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AspLanguageCommandIntent {
    Reasoning,
    ExactEvidence,
    DirectReadFallback,
    InvalidEvidence,
}

impl AspLanguageCommandIntent {
    /// Stable decision-field spelling for this intent.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reasoning => "reasoning",
            Self::ExactEvidence => "exact-evidence",
            Self::DirectReadFallback => "direct-read-fallback",
            Self::InvalidEvidence => "invalid-evidence",
        }
    }
}

/// Parsed public language-facade command used by hook and session policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspLanguageCommand {
    /// Language facade following `asp`, or supplied through `--language`.
    pub language_id: String,
    /// Semantic command intent.
    pub intent: AspLanguageCommandIntent,
    /// Stable normalized route such as `search-owner` or `query-selector`.
    pub route: String,
    /// Exact or attempted selector when the command supplied one.
    pub selector: Option<String>,
}

/// Parse one tokenized public `asp <language>` command without substring matching.
pub fn classify_asp_language_command_tokens(tokens: &[String]) -> Option<AspLanguageCommand> {
    classify_asp_language_command_tokens_with_policy(
        tokens,
        &HookClientAspCommandIntentPolicyConfig::default(),
    )
}

/// Parse one tokenized public command using the configured intent taxonomy.
pub fn classify_asp_language_command_tokens_with_policy(
    tokens: &[String],
    policy: &HookClientAspCommandIntentPolicyConfig,
) -> Option<AspLanguageCommand> {
    let asp_index = asp_invocation_indices(tokens).into_iter().next()?;
    let after_asp = &tokens[asp_index + 1..];
    let (language_id, command_tokens) = language_command_tokens(after_asp)?;
    let command = command_tokens.first()?.as_str();
    match command {
        "guide" if policy.reasoning.guide_command => Some(AspLanguageCommand {
            language_id,
            intent: AspLanguageCommandIntent::Reasoning,
            route: "guide".to_string(),
            selector: None,
        }),
        "search" => classify_search(language_id, command_tokens, policy),
        "query" => classify_query(language_id, command_tokens, policy),
        _ => None,
    }
}

fn language_command_tokens(tokens: &[String]) -> Option<(String, &[String])> {
    match tokens.first().map(String::as_str) {
        Some("guide" | "search" | "query") => {
            Some((option_value(tokens, "--language")?.to_string(), tokens))
        }
        Some(language_id) if !language_id.starts_with('-') => {
            Some((language_id.to_string(), tokens.get(1..)?))
        }
        _ => None,
    }
}

fn classify_search(
    language_id: String,
    tokens: &[String],
    policy: &HookClientAspCommandIntentPolicyConfig,
) -> Option<AspLanguageCommand> {
    let route_index = if tokens.get(1).map(String::as_str) == Some("--language") {
        3
    } else {
        1
    };
    let route = tokens.get(route_index)?.as_str();
    if !policy
        .reasoning
        .search_routes
        .iter()
        .any(|configured| configured == route)
    {
        return None;
    }
    Some(AspLanguageCommand {
        language_id,
        intent: AspLanguageCommandIntent::Reasoning,
        route: format!("search-{route}"),
        selector: None,
    })
}

fn classify_query(
    language_id: String,
    tokens: &[String],
    policy: &HookClientAspCommandIntentPolicyConfig,
) -> Option<AspLanguageCommand> {
    let selector = option_value(tokens, "--selector").map(str::to_string);
    if option_value(tokens, "--from-hook").is_some_and(|value| {
        policy
            .direct_read_fallback
            .from_hook_values
            .iter()
            .any(|configured| configured == value)
    }) {
        return Some(AspLanguageCommand {
            language_id,
            intent: AspLanguageCommandIntent::DirectReadFallback,
            route: "query-direct-read-fallback".to_string(),
            selector,
        });
    }
    if tokens.iter().any(|token| {
        policy
            .reasoning
            .query_flags
            .iter()
            .any(|configured| configured == token)
    }) {
        return Some(AspLanguageCommand {
            language_id,
            intent: AspLanguageCommandIntent::Reasoning,
            route: "query-reasoning".to_string(),
            selector,
        });
    }
    let projects_evidence = tokens.iter().any(|token| {
        policy
            .exact_evidence
            .query_projection_flags
            .iter()
            .any(|configured| configured == token)
    }) || option_value(tokens, "--view").is_some_and(|view| {
        policy
            .exact_evidence
            .query_projection_views
            .iter()
            .any(|configured| configured == view)
    });
    if projects_evidence {
        let exact_selector = selector.as_deref().is_some_and(|selector| {
            selector_is_parser_owned_for_language(&language_id, selector, policy)
        });
        let cross_language = selector.as_deref().is_some_and(|selector| {
            selector_language(selector)
                .is_some_and(|scheme| !selector_scheme_matches_language(scheme, &language_id))
        });
        let intent = if exact_selector {
            AspLanguageCommandIntent::ExactEvidence
        } else if policy
            .invalid_evidence
            .reject_projected_query_without_exact_selector
            || (cross_language && policy.invalid_evidence.reject_cross_language_selector)
        {
            AspLanguageCommandIntent::InvalidEvidence
        } else {
            AspLanguageCommandIntent::Reasoning
        };
        return Some(AspLanguageCommand {
            language_id,
            intent,
            route: "query-selector".to_string(),
            selector,
        });
    }
    policy
        .reasoning
        .unprojected_query
        .then_some(AspLanguageCommand {
            language_id,
            intent: AspLanguageCommandIntent::Reasoning,
            route: "query-reasoning".to_string(),
            selector,
        })
}

fn selector_is_parser_owned_for_language(
    language_id: &str,
    selector: &str,
    policy: &HookClientAspCommandIntentPolicyConfig,
) -> bool {
    if selector.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return false;
    }
    let Some((scheme, remainder)) = selector.split_once("://") else {
        return false;
    };
    if !valid_selector_scheme(scheme)
        || (policy.exact_evidence.require_same_language
            && !selector_scheme_matches_language(scheme, language_id))
        || remainder.contains("://")
    {
        return false;
    }
    let Some((owner, item_kind_and_name)) = remainder.split_once('#') else {
        return false;
    };
    let Some((kind, item)) = item_kind_and_name.split_once('/') else {
        return false;
    };
    if owner.is_empty()
        || owner.contains('#')
        || item.contains('#')
        || !policy
            .exact_evidence
            .selector_kinds
            .iter()
            .any(|configured| configured == kind)
    {
        return false;
    }
    let mut item_parts = item.split('/');
    matches!((item_parts.next(), item_parts.next()), (Some(item_kind), Some(name)) if !item_kind.is_empty() && !name.is_empty())
        && item_parts.all(|part| !part.is_empty())
}

fn selector_language(selector: &str) -> Option<&str> {
    selector.split_once("://").map(|(scheme, _)| scheme)
}

fn selector_scheme_matches_language(scheme: &str, language_id: &str) -> bool {
    scheme == language_id || (language_id == "gerbil-scheme" && scheme == "scheme")
}

fn valid_selector_scheme(scheme: &str) -> bool {
    let mut chars = scheme.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '+' | '-' | '.')
        })
}

fn option_value<'a>(tokens: &'a [String], option: &str) -> Option<&'a str> {
    tokens.windows(2).find_map(|pair| {
        (pair[0] == option && !pair[1].starts_with('-')).then_some(pair[1].as_str())
    })
}

/// Return parser-owned positions where `asp` is an invoked binary, not data.
pub fn asp_invocation_indices(tokens: &[String]) -> Vec<usize> {
    tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            (is_asp_binary_token(token) && is_asp_invocation_position(tokens, index))
                .then_some(index)
        })
        .collect()
}

fn is_asp_binary_token(token: &str) -> bool {
    token == "asp"
        || token.ends_with("/asp")
        || token.ends_with(".bin/asp")
        || token.ends_with("\\asp.exe")
}

fn is_asp_invocation_position(tokens: &[String], index: usize) -> bool {
    let stage_start = tokens[..index]
        .iter()
        .rposition(|token| is_shell_separator(token))
        .map_or(0, |separator| separator + 1);
    let mut command_index = stage_start;
    while tokens
        .get(command_index)
        .is_some_and(|token| is_env_assignment(token))
    {
        command_index += 1;
    }
    if command_index == index {
        return true;
    }

    let Some(wrapper) = tokens.get(command_index).map(|token| command_name(token)) else {
        return false;
    };
    if wrapper == "command" {
        return command_wrapper_invocation_index(tokens, command_index) == Some(index);
    }
    if matches!(wrapper, "exec" | "nohup") {
        return executable_after_options(tokens, command_index + 1) == Some(index);
    }
    false
}

fn command_wrapper_invocation_index(tokens: &[String], command_index: usize) -> Option<usize> {
    let mut index = command_index + 1;
    while let Some(token) = tokens.get(index).map(String::as_str) {
        match token {
            "-v" | "-V" => return None,
            "-p" => index += 1,
            "--" => return tokens.get(index + 1).map(|_| index + 1),
            value if value.starts_with('-') => return None,
            _ => return Some(index),
        }
    }
    None
}

fn executable_after_options(tokens: &[String], mut index: usize) -> Option<usize> {
    while let Some(token) = tokens.get(index).map(String::as_str) {
        match token {
            "--" => return tokens.get(index + 1).map(|_| index + 1),
            value if value.starts_with('-') => index += 1,
            _ => return Some(index),
        }
    }
    None
}

fn command_name(token: &str) -> &str {
    token.rsplit(['/', '\\']).next().unwrap_or(token)
}

fn is_shell_separator(token: &str) -> bool {
    matches!(token, "&&" | ";" | "|" | "||" | "&")
}

fn is_env_assignment(token: &str) -> bool {
    let Some((name, _)) = token.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric())
}
