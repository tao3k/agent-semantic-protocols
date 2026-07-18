use crate::protocol::normalize_source_route_selector;

use super::shell::is_separator;

pub(crate) fn path_like_tokens(tokens: &[String]) -> Vec<&str> {
    let mut paths = Vec::new();
    for token in tokens {
        path_like_token_matches(token, |path| {
            push_unique_path(&mut paths, path);
            false
        });
    }
    paths
}

pub(crate) fn command_source_paths(command: &str, tokens: &[String]) -> Vec<String> {
    let range_paths = crate::source_dump_range::line_range_source_paths(command);
    if !range_paths.is_empty() {
        let mut paths = range_paths;
        for range_path in paths.clone() {
            if let Some(base_path) = source_path_without_line_range(&range_path) {
                push_unique_owned_path(&mut paths, base_path.to_string());
            }
        }
        return paths;
    }
    let mut paths = Vec::new();
    for token in tokens {
        path_like_token_matches(token, |token| {
            push_command_source_path(&mut paths, token);
            false
        });
    }
    paths
}

pub(crate) fn path_like_token_matches<'a, F>(token: &'a str, mut predicate: F) -> bool
where
    F: FnMut(&'a str) -> bool,
{
    if !may_contain_path_like_fragment(token) {
        return false;
    }
    let normalized = normalize_source_route_selector(token);
    if is_path_like_token(normalized) {
        if predicate(normalized) {
            return true;
        }
        if normalized != token {
            return false;
        }
    }
    embedded_path_like_segment_matches(token, |segment| {
        let normalized = normalize_source_route_selector(segment);
        is_embedded_path_like_segment(normalized) && predicate(normalized)
    })
}

fn may_contain_path_like_fragment(token: &str) -> bool {
    token.contains(['/', '.', '*', '?', '{', '}', '[', ']', ':', '@'])
}

fn is_path_like_token(token: &str) -> bool {
    !token.starts_with('-')
        && !is_separator(token)
        && (token.contains('/') || token.contains('.') || token.contains('*'))
}

fn embedded_path_like_segment_matches<'a, F>(token: &'a str, mut predicate: F) -> bool
where
    F: FnMut(&'a str) -> bool,
{
    let mut start = None;
    for (index, character) in token.char_indices() {
        if is_path_fragment_character(character) {
            start.get_or_insert(index);
            continue;
        }
        if let Some(start_index) = start.take()
            && start_index < index
            && predicate(&token[start_index..index])
        {
            return true;
        }
    }
    if let Some(start_index) = start
        && start_index < token.len()
        && predicate(&token[start_index..])
    {
        return true;
    }
    false
}

fn is_path_fragment_character(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(
            character,
            '/' | '.' | '_' | '-' | '*' | '?' | '{' | '}' | '[' | ']' | ':' | '@'
        )
}

fn is_embedded_path_like_segment(segment: &str) -> bool {
    is_path_like_token(segment) && (!segment.starts_with('.') || segment.starts_with("../"))
}

fn push_unique_path<'a>(paths: &mut Vec<&'a str>, path: &'a str) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn push_command_source_path(paths: &mut Vec<String>, token: &str) {
    if token.contains(['{', '}', '[', ']', '?']) {
        push_unique_owned_path(paths, token.to_string());
        return;
    }
    let embedded = embedded_source_path_candidates(token);
    if embedded.is_empty() {
        if !looks_like_code_call_token(token) {
            push_unique_owned_path(paths, token.to_string());
        }
    } else {
        for path in embedded {
            push_unique_owned_path(paths, path);
        }
    }
}

fn looks_like_code_call_token(token: &str) -> bool {
    token.contains('(') || token.contains(')')
}

fn embedded_source_path_candidates(token: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut current = String::new();
    for ch in token.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | '*' | ':' | '~') {
            current.push(ch);
            continue;
        }
        if is_embedded_source_path_candidate(&current) {
            push_unique_owned_path(&mut paths, current.clone());
        }
        current.clear();
    }
    paths
}

fn is_embedded_source_path_candidate(candidate: &str) -> bool {
    !candidate.starts_with('-')
        && (candidate.contains('/') || candidate.contains('*'))
        && candidate.contains('.')
}

fn push_unique_owned_path(paths: &mut Vec<String>, path: String) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn source_path_without_line_range(path: &str) -> Option<&str> {
    let (base, suffix) = path.rsplit_once(':')?;
    if suffix.chars().all(|character| character.is_ascii_digit()) {
        let (base, start) = base.rsplit_once(':')?;
        return start
            .chars()
            .all(|character| character.is_ascii_digit())
            .then_some(base);
    }
    let (start, end) = suffix.split_once('-')?;
    (!start.is_empty()
        && !end.is_empty()
        && start.chars().all(|character| character.is_ascii_digit())
        && end.chars().all(|character| character.is_ascii_digit()))
    .then_some(base)
}
