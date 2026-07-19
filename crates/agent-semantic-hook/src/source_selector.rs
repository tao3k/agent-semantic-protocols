use crate::protocol::{normalize_source_route_selector, normalize_source_selector};
use crate::protocol_activation::protocol_activation_manifest::{
    ActivatedProvider, HookRuntime, ProviderSelectorMatch, SourceSelectorKind,
};

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
        for matched in matching_blocked_providers(registry, &route_selector, &should_block) {
            merge_source_selector_match(
                &mut matches,
                &route_selector,
                matched.provider,
                matched.kind,
            );
        }
    }
    matches
}

fn matching_blocked_providers<'provider, F>(
    registry: &'provider HookRuntime,
    route_selector: &str,
    should_block: &F,
) -> Vec<ProviderSelectorMatch<'provider>>
where
    F: Fn(&ActivatedProvider) -> bool,
{
    let match_selector = normalize_source_selector(route_selector);
    registry
        .providers_for_selector(match_selector)
        .into_iter()
        .filter(|matched| should_block(matched.provider))
        .collect()
}

fn merge_source_selector_match<'provider>(
    matches: &mut Vec<SourceSelectorMatch<'provider>>,
    route_selector: &str,
    provider: &'provider ActivatedProvider,
    kind: SourceSelectorKind,
) {
    if let Some(existing) = find_provider_match(matches, provider) {
        if selector_is_more_specific(&existing.route_selector, route_selector) {
            existing.route_selector = route_selector.to_string();
            existing.kind = kind;
        }
        return;
    }
    matches.push(SourceSelectorMatch {
        route_selector: route_selector.to_string(),
        provider,
        kind,
    });
}

fn find_provider_match<'matches, 'provider>(
    matches: &'matches mut [SourceSelectorMatch<'provider>],
    provider: &ActivatedProvider,
) -> Option<&'matches mut SourceSelectorMatch<'provider>> {
    matches.iter_mut().find(|existing| {
        existing.provider.language_id == provider.language_id
            && existing.provider.provider_id == provider.provider_id
    })
}

fn selector_is_more_specific(existing: &str, candidate: &str) -> bool {
    source_selector_base(existing) == source_selector_base(candidate)
        && selector_specificity(candidate) > selector_specificity(existing)
}

fn selector_specificity(selector: &str) -> u8 {
    u8::from(selector != source_selector_base(selector))
}

pub(crate) fn source_selector_base(selector: &str) -> &str {
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
