//! Shell command normalization and semantic-search routing helpers.

use crate::protocol::{LanguageProfile, ProfileRegistry};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum CommandIntent {
    Other,
    DirectRead,
    ContentDump,
    RawSearch,
}

fn shell_tokens(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '\'' | '"') => quote = Some(ch),
            (None, '|' | ';' | '&') => {
                push_token(&mut tokens, &mut current);
                if ch == '&' && chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push("&&".to_string());
                } else {
                    tokens.push(ch.to_string());
                }
            }
            (None, c) if c.is_whitespace() => push_token(&mut tokens, &mut current),
            (None, c) => current.push(c),
        }
    }
    push_token(&mut tokens, &mut current);
    tokens
}

pub(crate) fn semantic_shell_tokens(command: &str) -> Vec<String> {
    split_command_stages(shell_tokens(command))
        .into_iter()
        .flat_map(|stage| unwrap_command_stage(&stage))
        .collect()
}

fn split_command_stages(tokens: Vec<String>) -> Vec<Vec<String>> {
    let mut stages = Vec::new();
    let mut stage = Vec::new();
    for token in tokens {
        if is_separator(&token) {
            if !stage.is_empty() {
                stages.push(std::mem::take(&mut stage));
            }
            stages.push(vec![token]);
        } else {
            stage.push(token);
        }
    }
    if !stage.is_empty() {
        stages.push(stage);
    }
    stages
}

fn unwrap_command_stage(tokens: &[String]) -> Vec<String> {
    let tokens = strip_env_assignments(tokens);
    if tokens.is_empty() {
        return Vec::new();
    }
    match tokens[0].as_str() {
        "env" => {
            return unwrap_command_stage(&tokens[env_command_index(tokens)..]);
        }
        "direnv" if tokens.get(1).map(String::as_str) == Some("exec") => {
            if tokens.len() > 3 {
                return unwrap_command_stage(&tokens[3..]);
            }
        }
        "rtk" => {
            return unwrap_rtk_stage(tokens);
        }
        "uv" if tokens.get(1).map(String::as_str) == Some("run") => {
            return unwrap_command_stage(&tokens[uv_run_command_index(tokens)..]);
        }
        "cargo" => {
            if let Some(index) = tokens.iter().position(|token| token == "--") {
                return unwrap_command_stage(&tokens[index + 1..]);
            }
        }
        "bash" | "sh" | "zsh" => {
            if let Some(index) = tokens
                .iter()
                .position(|token| matches!(token.as_str(), "-c" | "-lc"))
            {
                if let Some(script) = tokens.get(index + 1) {
                    return semantic_shell_tokens(script);
                }
            }
        }
        _ => {}
    }
    tokens.to_vec()
}

fn strip_env_assignments(tokens: &[String]) -> &[String] {
    let mut index = 0;
    while tokens
        .get(index)
        .is_some_and(|token| is_env_assignment(token))
    {
        index += 1;
    }
    &tokens[index..]
}

fn is_env_assignment(token: &str) -> bool {
    let Some((name, _)) = token.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn env_command_index(tokens: &[String]) -> usize {
    let mut index = 1;
    while index < tokens.len() {
        match tokens[index].as_str() {
            "-i" | "--ignore-environment" => index += 1,
            "-u" | "--unset" | "-C" | "--chdir" => index += 2,
            value
                if value.starts_with("-u")
                    || value.starts_with("--unset=")
                    || value.starts_with("-C")
                    || value.starts_with("--chdir=")
                    || is_env_assignment(value) =>
            {
                index += 1;
            }
            _ => break,
        }
    }
    index
}

fn unwrap_rtk_stage(tokens: &[String]) -> Vec<String> {
    let command_index = tokens
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, token)| (!token.starts_with('-')).then_some(index))
        .unwrap_or(tokens.len());
    let Some(command) = tokens.get(command_index).map(String::as_str) else {
        return Vec::new();
    };
    match command {
        "run" => {
            if let Some(script_index) = tokens[command_index + 1..]
                .iter()
                .position(|token| matches!(token.as_str(), "-c" | "-lc"))
                .map(|offset| command_index + 1 + offset + 1)
            {
                if let Some(script) = tokens.get(script_index) {
                    return semantic_shell_tokens(script);
                }
            }
            unwrap_command_stage(&tokens[command_index + 1..])
        }
        "proxy" | "err" | "summary" => unwrap_command_stage(&tokens[command_index + 1..]),
        _ => unwrap_command_stage(&tokens[command_index..]),
    }
}

fn uv_run_command_index(tokens: &[String]) -> usize {
    let mut index = 2;
    while index < tokens.len() {
        match tokens[index].as_str() {
            "--project" | "--directory" | "--with" | "--python" => index += 2,
            "--frozen" | "--locked" | "--isolated" | "--no-sync" => index += 1,
            value
                if value.starts_with("--project=")
                    || value.starts_with("--directory=")
                    || value.starts_with("--with=")
                    || value.starts_with("--python=") =>
            {
                index += 1;
            }
            _ => break,
        }
    }
    index
}

fn push_token(tokens: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        tokens.push(std::mem::take(current));
    }
}

pub(crate) fn command_intent(tokens: &[String]) -> CommandIntent {
    let command = first_stage_command(tokens);
    if matches!(command.as_deref(), Some("read")) {
        return CommandIntent::DirectRead;
    }
    if matches!(
        command.as_deref(),
        Some("cat" | "sed" | "nl" | "bat" | "head" | "tail" | "awk" | "less")
    ) {
        return CommandIntent::ContentDump;
    }
    if matches!(
        command.as_deref(),
        Some("rg" | "grep" | "ag" | "fd" | "find" | "git")
    ) {
        return CommandIntent::RawSearch;
    }
    CommandIntent::Other
}

fn first_stage_command(tokens: &[String]) -> Option<String> {
    tokens
        .iter()
        .find(|token| !token.starts_with('-') && !is_separator(token))
        .cloned()
}

pub(crate) fn first_path(tokens: &[String]) -> Option<&str> {
    tokens.iter().find_map(|token| {
        if token.starts_with('-') || is_separator(token) {
            return None;
        }
        if token.contains('/') || token.contains('.') {
            Some(token.as_str())
        } else {
            None
        }
    })
}

pub(crate) fn profiles_for_command<'a>(
    registry: &'a ProfileRegistry,
    tokens: &[String],
) -> Vec<&'a LanguageProfile> {
    if tokens.iter().any(|token| token == ".") {
        return registry.profiles.iter().collect();
    }
    let mut profiles = Vec::new();
    for token in tokens {
        for profile in registry
            .profiles
            .iter()
            .filter(|profile| profile.matches_search_token(token))
        {
            if !profiles
                .iter()
                .any(|existing: &&LanguageProfile| existing.language_id == profile.language_id)
            {
                profiles.push(profile);
            }
        }
    }
    profiles
}

pub(crate) fn profiles_for_raw_search<'a>(
    registry: &'a ProfileRegistry,
    tokens: &[String],
) -> Vec<&'a LanguageProfile> {
    let targets = explicit_raw_search_targets(tokens);
    if !targets.is_empty() {
        let mut profiles = Vec::new();
        for profile in &registry.profiles {
            if targets.matches(profile) {
                profiles.push(profile);
            }
        }
        return profiles;
    }
    profiles_for_command(registry, tokens)
}

#[derive(Default)]
struct RawSearchTargets {
    extensions: Vec<String>,
    types: Vec<String>,
}

impl RawSearchTargets {
    fn is_empty(&self) -> bool {
        self.extensions.is_empty() && self.types.is_empty()
    }

    fn normalize(&mut self) {
        self.extensions.sort();
        self.extensions.dedup();
        self.types.sort();
        self.types.dedup();
    }

    fn matches(&self, profile: &LanguageProfile) -> bool {
        self.extensions.iter().any(|extension| {
            profile
                .source_extensions
                .iter()
                .any(|source| source == extension)
        }) || self.types.iter().any(|target_type| {
            target_type == &profile.language_id
                || target_type == &profile.namespace
                || profile
                    .source_extensions
                    .iter()
                    .any(|source| source.trim_start_matches('.') == target_type)
        })
    }
}

fn explicit_raw_search_targets(tokens: &[String]) -> RawSearchTargets {
    let mut targets = RawSearchTargets::default();
    for (index, token) in tokens.iter().enumerate() {
        let next = tokens.get(index + 1);
        if matches!(token.as_str(), "-e" | "--extension" | "--ext") {
            if let Some(value) = next {
                push_extension(&mut targets.extensions, value, true);
            }
        }
        if matches!(token.as_str(), "-t" | "--type") {
            if let Some(value) = next {
                push_type(&mut targets.types, value);
                push_extension(&mut targets.extensions, value, true);
            }
        }
        if matches!(token.as_str(), "-g" | "--glob" | "-name" | "--name") {
            if let Some(value) = next {
                push_extension(&mut targets.extensions, value, false);
            }
        }
        if let Some(value) = token
            .strip_prefix("--glob=")
            .or_else(|| token.strip_prefix("--name="))
            .or_else(|| token.strip_prefix("--extension="))
            .or_else(|| token.strip_prefix("--ext="))
        {
            push_extension(&mut targets.extensions, value, true);
        }
        if let Some(value) = token.strip_prefix("--type=") {
            push_type(&mut targets.types, value);
            push_extension(&mut targets.extensions, value, true);
        }
        push_extension(&mut targets.extensions, token, false);
    }
    targets.normalize();
    targets
}

fn push_type(types: &mut Vec<String>, token: &str) {
    let clean = token
        .trim_matches(|character| matches!(character, '\'' | '"' | ',' | ';'))
        .trim_start_matches("type:")
        .to_ascii_lowercase();
    if !clean.is_empty()
        && clean.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        types.push(clean);
    }
}

fn push_extension(extensions: &mut Vec<String>, token: &str, allow_bare: bool) {
    let clean = token
        .trim_matches(|character| matches!(character, '\'' | '"' | ',' | ';'))
        .trim_start_matches('*')
        .to_ascii_lowercase();
    if let Some(start) = clean.find(".{") {
        if let Some(end) = clean[start + 2..].find('}') {
            for extension in clean[start + 2..start + 2 + end].split(',') {
                if !extension.is_empty()
                    && extension
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric())
                {
                    extensions.push(format!(".{extension}"));
                }
            }
            return;
        }
    }
    let clean = clean.trim_start_matches('{').trim_end_matches('}');
    if allow_bare
        && clean
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        extensions.push(format!(".{clean}"));
        return;
    }
    if let Some((_, extension)) = clean.rsplit_once('.') {
        let extension = extension.trim_end_matches('}');
        if !extension.is_empty()
            && extension
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        {
            extensions.push(format!(".{extension}"));
        }
    }
}

pub(crate) fn contains_ingest_pipe(tokens: &[String], profiles: &[&LanguageProfile]) -> bool {
    profiles.iter().any(|profile| {
        tokens.windows(3).any(|window| {
            window[0] == profile.binary && window[1] == "search" && window[2] == "ingest"
        })
    })
}

pub(crate) fn search_json_route<'a>(
    registry: &'a ProfileRegistry,
    tokens: &[String],
) -> Option<(&'a LanguageProfile, Vec<String>)> {
    for profile in &registry.profiles {
        let Some(binary_index) = tokens.iter().position(|token| token == &profile.binary) else {
            continue;
        };
        if tokens.get(binary_index + 1).map(String::as_str) != Some("search") {
            continue;
        }
        let mut argv = tokens[binary_index..]
            .iter()
            .take_while(|token| !is_separator(token))
            .filter(|token| token.as_str() != "--json")
            .cloned()
            .collect::<Vec<_>>();
        if !argv.iter().any(|arg| arg == "--view") {
            let insert_at = argv
                .iter()
                .rposition(|arg| arg == ".")
                .unwrap_or(argv.len());
            argv.splice(
                insert_at..insert_at,
                ["--view".to_string(), "seeds".to_string()],
            );
        }
        return Some((profile, argv));
    }
    None
}

fn is_separator(token: &str) -> bool {
    matches!(token, "|" | ";" | "&&" | "&")
}
