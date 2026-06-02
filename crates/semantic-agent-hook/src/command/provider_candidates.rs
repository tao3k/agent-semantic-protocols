use crate::protocol::normalize_source_route_selector;
use crate::protocol_activation::ActivatedProvider;

use super::shell::is_separator;

pub(crate) fn path_like_tokens(tokens: &[String]) -> Vec<&str> {
    tokens
        .iter()
        .filter_map(|token| {
            let normalized = normalize_source_route_selector(token);
            if is_path_like_token(normalized) {
                Some(normalized)
            } else {
                None
            }
        })
        .collect()
}

fn is_path_like_token(token: &str) -> bool {
    !token.starts_with('-')
        && !is_separator(token)
        && (token.contains('/') || token.contains('.') || token.contains('*'))
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
