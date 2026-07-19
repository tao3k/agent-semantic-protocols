//! Parser-owned classification for public `asp <language>` commands.

use agent_semantic_config::HookClientAspCommandIntentPolicyConfig;

/// Parsed public language-facade command used by hook and session policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspLanguageCommand {
    /// Language facade following `asp`, or supplied through `--language`.
    pub language_id: String,
    /// Semantic command intent.
    pub intent: agent_semantic_config::AspCommandIntent,
    /// Canonical typed route; serialize it only at the decision boundary.
    pub route: agent_semantic_config::AspCommandRouteId,
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
    let matched =
        agent_semantic_config::classify_asp_language_command(language_id, command_tokens, policy)?;
    Some(AspLanguageCommand {
        language_id: matched.language_id,
        intent: matched.intent,
        route: matched.route,
        selector: matched.selector,
    })
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
