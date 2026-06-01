use crate::protocol::LanguageProfile;

use super::shell::is_separator;

pub(crate) fn path_like_tokens(tokens: &[String]) -> Vec<&str> {
    tokens
        .iter()
        .filter_map(|token| {
            if is_path_like_token(token) {
                Some(token.as_str())
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

pub(super) fn push_profile_once<'a>(
    profiles: &mut Vec<&'a LanguageProfile>,
    profile: &'a LanguageProfile,
) {
    if !profiles
        .iter()
        .any(|existing| existing.language_id == profile.language_id)
    {
        profiles.push(profile);
    }
}
