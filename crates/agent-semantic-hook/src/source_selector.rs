use crate::protocol::normalize_source_route_selector;
use crate::protocol::normalize_source_selector;
use crate::protocol_activation::protocol_activation_manifest::HookProviderProjection;
use crate::protocol_activation::protocol_activation_manifest::HookRuntime;
use crate::protocol_activation::protocol_activation_manifest::ProviderSelectorMatch;
use crate::protocol_activation::protocol_activation_manifest::SourceSelectorKind;

pub(crate) struct SourceSelectorMatch {
    pub(crate) route_selector: String,
    pub(crate) provider: HookProviderProjection,
    pub(crate) kind: SourceSelectorKind,
}

pub(crate) fn derive_agent_action_subjects(
    registry: &HookRuntime,
    paths: &[String],
) -> Vec<crate::tool_action::AgentActionSubject> {
    let (mut semantic_subjects, other_subjects): (Vec<_>, Vec<_>) = paths
        .iter()
        .map(|path| crate::tool_action::AgentActionSubject {
            value: path.clone(),
            kind: infer_agent_action_subject_kind(registry, path),
        })
        .partition(|subject| {
            !matches!(
                &subject.kind,
                crate::tool_action::AgentActionSubjectKind::Other
            )
        });
    semantic_subjects.extend(other_subjects);
    semantic_subjects
}

pub(crate) fn project_shell_subject_paths(registry: &HookRuntime, paths: &[String]) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut projected = Vec::new();
    for path in paths {
        if path.starts_with('-') || !is_path_operand(registry, path) {
            continue;
        }
        if seen.insert(path.clone()) {
            projected.push(path.clone());
        }
    }
    projected
}

fn is_path_operand(registry: &HookRuntime, value: &str) -> bool {
    if !matches!(
        infer_agent_action_subject_kind(registry, value),
        crate::tool_action::AgentActionSubjectKind::Other
    ) {
        return true;
    }

    let leaf = value.rsplit(['/', '\\']).next().unwrap_or(value);
    let Some((stem, suffix)) = leaf.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && !suffix.is_empty()
        && suffix
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '*' | '?' | '[' | ']' | '-'))
}

fn infer_agent_action_subject_kind(
    registry: &HookRuntime,
    value: &str,
) -> crate::tool_action::AgentActionSubjectKind {
    use crate::tool_action::AgentActionSubjectKind;

    if value.contains("://") || value.contains("#item/") {
        return AgentActionSubjectKind::StructuralSelector;
    }
    if value == "." || value == ".." {
        return AgentActionSubjectKind::Directory;
    }

    let normalized = normalize_source_selector(value);
    let leaf = value.rsplit(['/', '\\']).next().unwrap_or(value);
    let is_path_shaped = value.contains(['/', '\\']) && !value.chars().any(char::is_whitespace);
    let registered_source_scope =
        crate::protocol_activation::provider_routing::hook_provider_projections(registry)
            .iter()
            .any(|provider| {
                let ignored = std::iter::empty::<&String>().any(|prefix| {
                    normalized == prefix
                        || normalized
                            .strip_prefix(prefix)
                            .is_some_and(|suffix| suffix.starts_with('/'))
                });
                !ignored
                    && provider.package_roots.iter().any(|root| {
                        root == "."
                            || normalized == root
                            || normalized
                                .strip_prefix(root)
                                .is_some_and(|suffix| suffix.starts_with('/'))
                            || contains_path_component_sequence(normalized, root)
                    })
            });
    let registered_root_alias = !normalized.contains('/')
        && crate::protocol_activation::provider_routing::hook_provider_projections(registry)
            .iter()
            .any(|provider| {
                provider.package_roots.iter().any(|root| {
                    root != "."
                        && root
                            .trim_end_matches(['/', '\\'])
                            .rsplit(['/', '\\'])
                            .next()
                            .is_some_and(|root_leaf| root_leaf == normalized)
                })
            });

    if (registered_source_scope || registered_root_alias)
        && (is_path_shaped || registered_root_alias)
        && (value.ends_with(['/', '\\']) || !leaf.contains('.'))
    {
        return AgentActionSubjectKind::RegisteredLanguageSourcePattern;
    }
    if value.ends_with(['/', '\\']) {
        return AgentActionSubjectKind::Directory;
    }

    let registered =
        !collect_source_selector_matches(registry, std::iter::once(value), |_| true).is_empty();
    if !registered {
        return AgentActionSubjectKind::Other;
    }
    if leaf.chars().any(|ch| matches!(ch, '*' | '?' | '[' | ']')) {
        AgentActionSubjectKind::RegisteredLanguageSourcePattern
    } else {
        AgentActionSubjectKind::RegisteredLanguageSource
    }
}

fn contains_path_component_sequence(path: &str, sequence: &str) -> bool {
    path.match_indices(sequence).any(|(start, matched)| {
        let end = start + matched.len();
        (start == 0 || path.as_bytes().get(start.wrapping_sub(1)) == Some(&b'/'))
            && (end == path.len() || path.as_bytes().get(end) == Some(&b'/'))
    })
}

pub(crate) fn collect_source_selector_matches<I, S, F>(
    registry: &HookRuntime,
    selectors: I,
    should_block: F,
) -> Vec<SourceSelectorMatch>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    F: Fn(&HookProviderProjection) -> bool,
{
    let mut matches: Vec<SourceSelectorMatch> = Vec::new();
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

fn matching_blocked_providers<F>(
    registry: &HookRuntime,
    route_selector: &str,
    should_block: &F,
) -> Vec<ProviderSelectorMatch>
where
    F: Fn(&HookProviderProjection) -> bool,
{
    let match_selector = normalize_source_selector(route_selector);
    registry
        .providers_for_selector(match_selector)
        .into_iter()
        .filter(|matched| should_block(&matched.provider))
        .collect()
}

fn merge_source_selector_match(
    matches: &mut Vec<SourceSelectorMatch>,
    route_selector: &str,
    provider: HookProviderProjection,
    kind: SourceSelectorKind,
) {
    if let Some(existing) = find_provider_match(matches, &provider) {
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

fn find_provider_match<'matches>(
    matches: &'matches mut [SourceSelectorMatch],
    provider: &HookProviderProjection,
) -> Option<&'matches mut SourceSelectorMatch> {
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

#[cfg(test)]
#[path = "../tests/unit/source_selector.rs"]
mod tests;
