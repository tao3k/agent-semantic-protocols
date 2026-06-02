use crate::protocol::{normalize_source_route_selector, normalize_source_selector};
use crate::protocol_activation::{ActivatedProvider, HookRuntime, SourceSelectorKind};

pub(crate) struct SourceSelectorMatch<'provider> {
    pub(crate) route_selector: String,
    pub(crate) provider: &'provider ActivatedProvider,
    pub(crate) kind: SourceSelectorKind,
}

pub(crate) fn collect_source_selector_matches<'provider, I, S, F>(
    registry: &'provider HookRuntime,
    selectors: I,
    should_block: F,
) -> Vec<SourceSelectorMatch<'provider>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    F: Fn(&ActivatedProvider) -> bool,
{
    let mut matches: Vec<SourceSelectorMatch<'provider>> = Vec::new();
    for selector in selectors {
        let route_selector = normalize_source_route_selector(selector.as_ref()).to_string();
        let match_selector = normalize_source_selector(&route_selector);
        for matched in registry.providers_for_selector(match_selector) {
            if !should_block(matched.provider) {
                continue;
            }
            if let Some(existing) = matches.iter_mut().find(|existing| {
                existing.provider.language_id == matched.provider.language_id
                    && existing.provider.provider_id == matched.provider.provider_id
            }) {
                if selector_is_more_specific(&existing.route_selector, &route_selector) {
                    existing.route_selector = route_selector.clone();
                    existing.kind = matched.kind;
                }
                continue;
            }
            matches.push(SourceSelectorMatch {
                route_selector: route_selector.clone(),
                provider: matched.provider,
                kind: matched.kind,
            });
        }
    }
    matches
}

fn selector_is_more_specific(existing: &str, candidate: &str) -> bool {
    source_selector_base(existing) == source_selector_base(candidate)
        && selector_specificity(candidate) > selector_specificity(existing)
}

fn selector_specificity(selector: &str) -> u8 {
    u8::from(selector != source_selector_base(selector))
}

fn source_selector_base(selector: &str) -> &str {
    let mut base = normalize_source_selector(selector);
    while let Some((path, suffix)) = base.rsplit_once(':') {
        if !is_line_locator_suffix(suffix) {
            break;
        }
        base = path;
    }
    base
}

fn is_line_locator_suffix(value: &str) -> bool {
    if let Some((start, end)) = value.split_once('-') {
        is_decimal_locator(start) && is_decimal_locator(end)
    } else {
        is_decimal_locator(value)
    }
}

fn is_decimal_locator(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

pub(crate) fn provider_source_selector(provider: &ActivatedProvider) -> String {
    let mut extensions = provider
        .source_extensions
        .iter()
        .map(|extension| extension.trim_start_matches('.').to_string())
        .filter(|extension| !extension.is_empty())
        .collect::<Vec<_>>();
    extensions.sort();
    extensions.dedup();
    match extensions.as_slice() {
        [] => "**/*".to_string(),
        [extension] => format!("**/*.{extension}"),
        extensions => format!("**/*.{{{}}}", extensions.join(",")),
    }
}

pub(crate) fn provider_matches_source_extension(
    provider: &ActivatedProvider,
    extension: &str,
) -> bool {
    provider
        .source_extensions
        .iter()
        .any(|source| source == extension)
}

pub(crate) fn provider_matches_source_type(
    provider: &ActivatedProvider,
    target_type: &str,
) -> bool {
    target_type == provider.language_id
        || target_type == provider.namespace
        || provider
            .source_extensions
            .iter()
            .any(|source| source.trim_start_matches('.') == target_type)
}

pub(crate) fn push_source_extension(extensions: &mut Vec<String>, token: &str, allow_bare: bool) {
    let clean = token
        .trim_matches(|character| matches!(character, '\'' | '"' | ',' | ';'))
        .trim_start_matches('*')
        .to_ascii_lowercase();
    if let Some(start) = clean.find(".{")
        && let Some(end) = clean[start + 2..].find('}')
    {
        for extension in clean[start + 2..start + 2 + end].split(',') {
            if is_source_extension_atom(extension) {
                extensions.push(format!(".{extension}"));
            }
        }
        return;
    }
    let clean = clean.trim_start_matches('{').trim_end_matches('}');
    if allow_bare && is_source_extension_atom(clean) {
        extensions.push(format!(".{clean}"));
        return;
    }
    if let Some((_, extension)) = clean.rsplit_once('.') {
        let extension = extension.trim_end_matches('}');
        if is_source_extension_atom(extension) {
            extensions.push(format!(".{extension}"));
        }
    }
}

pub(crate) fn selector_has_glob(token: &str) -> bool {
    token
        .chars()
        .any(|character| matches!(character, '*' | '?' | '[' | ']' | '{' | '}'))
}

fn is_source_extension_atom(extension: &str) -> bool {
    !extension.is_empty()
        && extension
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}
