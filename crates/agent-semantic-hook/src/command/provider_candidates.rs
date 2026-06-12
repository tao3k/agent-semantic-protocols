use crate::protocol::normalize_source_route_selector;
use crate::protocol_activation::ActivatedProvider;

use super::shell::is_separator;

pub(crate) fn path_like_tokens(tokens: &[String]) -> Vec<&str> {
    let mut paths = Vec::new();
    for token in tokens {
        let normalized = normalize_source_route_selector(token);
        if is_path_like_token(normalized) {
            push_unique_path(&mut paths, normalized);
            if normalized != token {
                continue;
            }
        }
        for segment in embedded_path_like_segments(token) {
            let normalized = normalize_source_route_selector(segment);
            if is_embedded_path_like_segment(normalized) {
                push_unique_path(&mut paths, normalized);
            }
        }
    }
    paths
}

fn is_path_like_token(token: &str) -> bool {
    !token.starts_with('-')
        && !is_separator(token)
        && (token.contains('/') || token.contains('.') || token.contains('*'))
}

fn embedded_path_like_segments(token: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = None;
    for (index, character) in token.char_indices() {
        if is_path_fragment_character(character) {
            start.get_or_insert(index);
            continue;
        }
        if let Some(start_index) = start.take()
            && start_index < index
        {
            segments.push(&token[start_index..index]);
        }
    }
    if let Some(start_index) = start
        && start_index < token.len()
    {
        segments.push(&token[start_index..]);
    }
    segments
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

pub(super) fn push_provider_once<'a>(
    providers: &mut Vec<&'a ActivatedProvider>,
    provider: &'a ActivatedProvider,
) {
    if !providers
        .iter()
        .any(|existing| existing.language_id == provider.language_id)
    {
        providers.push(provider);
    }
}
