//! Source-access, direct-read, and raw-search classifier routes.

use crate::command::{
    CommandIntent, command_intent, command_source_paths, contains_ingest_pipe,
    looks_like_command_transcript, path_like_tokens, raw_search_plan, search_query_route,
    search_query_route_for_selector, selector_query_route,
};
use crate::hook_recovery_prompt::CompiledRecoveryPromptConfig;
use crate::source_selector::source_selector_base;
use crate::{
    ActivatedProvider, DecisionRoute, DecisionRouteKind, HookDecision, HookRuntime,
    OperationIntent, ReasonKind, SourceSelectorKind, SourceSelectorMatch, ToolAction,
    collect_source_selector_matches, subject_for_action,
};

use super::decision::deny_for_action;
use super::inline_source_read;
use super::recovery::source_access_recovery_message;

pub(super) fn classify_direct_read_action(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    semantic_ast_patch_enabled: bool,
    recovery_prompt: &CompiledRecoveryPromptConfig,
) -> Option<HookDecision> {
    fn directory_path_matches_provider(
        project_root: &str,
        provider: &ActivatedProvider,
        path: &str,
    ) -> bool {
        let Some(normalized) = normalize_directory_path(path) else {
            return false;
        };

        if let Some(project_relative) = project_relative_directory(project_root, &normalized) {
            return directory_path_candidate_matches_provider(provider, &project_relative);
        }

        if (project_root.trim().is_empty() || project_root.trim() == ".")
            && let Ok(current_dir) = std::env::current_dir()
            && let Some(current_dir) = current_dir.to_str()
            && let Some(project_relative) = project_relative_directory(current_dir, &normalized)
        {
            return directory_path_candidate_matches_provider(provider, &project_relative);
        }

        if !std::path::Path::new(&normalized).is_absolute() {
            return directory_path_candidate_matches_provider(provider, &normalized);
        }

        provider.matches_search_token(&normalized)
    }

    fn normalize_directory_path(path: &str) -> Option<String> {
        let path = path.trim().trim_start_matches("./").trim_end_matches('/');
        if path.is_empty() {
            return None;
        }

        let absolute = path.starts_with('/');
        let segments = path
            .split('/')
            .try_fold(Vec::new(), |mut segments, segment| {
                match segment {
                    "" | "." => {}
                    ".." => {
                        segments.pop()?;
                    }
                    segment => segments.push(segment),
                }
                Some(segments)
            })?;

        let joined = segments.join("/");
        if absolute {
            Some(format!("/{joined}"))
        } else if joined.is_empty() {
            Some(".".to_string())
        } else {
            Some(joined)
        }
    }

    fn project_relative_directory(project_root: &str, normalized: &str) -> Option<String> {
        let project_root = normalize_directory_path(project_root)?;
        if project_root == "." {
            return None;
        }
        if normalized == project_root {
            return Some(".".to_string());
        }
        normalized
            .strip_prefix(&project_root)
            .and_then(|path| path.strip_prefix('/'))
            .filter(|path| !path.is_empty())
            .map(str::to_string)
    }

    fn directory_path_candidate_matches_provider(
        provider: &ActivatedProvider,
        candidate: &str,
    ) -> bool {
        if provider.matches_search_token(candidate) {
            return true;
        }

        provider.source_roots.iter().any(|root| {
            let Some(root) = normalize_directory_path(root) else {
                return false;
            };
            !root.is_empty()
                && (candidate == root
                    || candidate.starts_with(&format!("{root}/"))
                    || candidate.ends_with(&format!("/{root}"))
                    || candidate.contains(&format!("/{root}/")))
        })
    }

    if action.operation == OperationIntent::DirectoryRead {
        let providers = registry
            .providers
            .iter()
            .filter(|provider| provider.policy.blocks_raw_source_search())
            .filter(|provider| {
                action.paths.iter().any(|path| {
                    directory_path_matches_provider(&registry.project_root, provider, path)
                })
            })
            .collect::<Vec<_>>();

        if providers.is_empty() {
            return None;
        }

        let routes: Vec<DecisionRoute> = providers
            .iter()
            .map(|provider| raw_search_ingest_route(provider))
            .collect();
        let message = source_access_recovery_message(
            platform,
            "source-directory-enumeration",
            &providers,
            &routes,
            semantic_ast_patch_enabled,
            recovery_prompt,
        );
        return Some(deny_for_action(
            platform,
            event,
            ReasonKind::SourceDirectoryEnumeration,
            action,
            language_ids(&providers),
            subject_for_action(action),
            routes,
            message,
        ));
    }

    if !action.operation.is_direct_read_candidate()
        && !action
            .command
            .as_deref()
            .is_some_and(looks_like_command_transcript)
    {
        return None;
    }
    let direct_read_selectors = if action.operation.is_direct_read_candidate() {
        action.paths.clone()
    } else {
        action
            .command
            .as_deref()
            .zip(action.command_tokens().as_deref())
            .map(|(command, tokens)| command_source_paths(command, tokens))
            .unwrap_or_default()
    };
    direct_read_decision(
        registry,
        platform,
        event,
        action,
        semantic_ast_patch_enabled,
        recovery_prompt,
        collect_direct_read_matches(registry, direct_read_selectors.iter().map(String::as_str)),
    )
}

pub(super) struct SourceReadCommandRequest<'a> {
    pub(super) registry: &'a HookRuntime,
    pub(super) platform: &'a str,
    pub(super) event: &'a str,
    pub(super) action: &'a ToolAction,
    pub(super) command: &'a str,
    pub(super) tokens: &'a [String],
    pub(super) semantic_ast_patch_enabled: bool,
    pub(super) recovery_prompt: &'a CompiledRecoveryPromptConfig,
}

pub(super) fn classify_source_read_command(
    request: SourceReadCommandRequest<'_>,
) -> Option<HookDecision> {
    let intent = command_intent(request.tokens);
    if intent == CommandIntent::VcsDiffReview {
        return None;
    }
    let mut inline_source_read_paths =
        inline_source_read::source_read_paths(request.command, request.tokens);
    append_selector_base_paths_for_ranges(&mut inline_source_read_paths);
    if !inline_source_read_paths.is_empty() {
        let matches = collect_content_dump_matches(
            request.registry,
            inline_source_read_paths.iter().map(String::as_str),
        );
        if !matches.is_empty() {
            return Some(content_dump_decision(
                request.platform,
                request.event,
                request.action,
                matches,
                Some(inline_source_read_paths),
                request.semantic_ast_patch_enabled,
                request.recovery_prompt,
            ));
        }
    }

    match intent {
        CommandIntent::DirectRead => direct_read_decision(
            request.registry,
            request.platform,
            request.event,
            request.action,
            request.semantic_ast_patch_enabled,
            request.recovery_prompt,
            collect_direct_read_matches(
                request.registry,
                action_path_selectors(request.action, request.tokens),
            ),
        ),
        CommandIntent::ContentDump => {
            let selectors = action_path_selectors(request.action, request.tokens);
            let matches = collect_content_dump_matches(
                request.registry,
                selectors.iter().map(String::as_str),
            );
            if matches.is_empty() {
                return None;
            }
            let subject_paths = selectors;
            Some(content_dump_decision(
                request.platform,
                request.event,
                request.action,
                matches,
                Some(subject_paths),
                request.semantic_ast_patch_enabled,
                request.recovery_prompt,
            ))
        }
        _ => None,
    }
}

fn action_path_selectors(action: &ToolAction, tokens: &[String]) -> Vec<String> {
    let mut selectors = Vec::new();
    for path in &action.paths {
        push_unique_selector(&mut selectors, path);
    }
    if !action.paths.is_empty() {
        append_selector_base_paths_for_ranges(&mut selectors);
        return selectors;
    }
    if let Some(command) = action.command.as_deref() {
        for path in command_source_paths(command, tokens) {
            push_unique_selector(&mut selectors, &path);
        }
    } else {
        for path in path_like_tokens(tokens) {
            push_unique_selector(&mut selectors, path);
        }
    }
    selectors
}

fn append_selector_base_paths_for_ranges(selectors: &mut Vec<String>) {
    for selector in selectors.clone() {
        if let Some(base) = selector_without_line_range(&selector)
            && !selectors.iter().any(|selector| selector == base)
        {
            selectors.push(base.to_string());
        }
    }
}

fn selector_without_line_range(selector: &str) -> Option<&str> {
    let (base, suffix) = selector.rsplit_once(':')?;
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

fn push_unique_selector(selectors: &mut Vec<String>, selector: &str) {
    let selector_base = source_selector_base(selector).to_string();
    if let Some(existing) = selectors
        .iter_mut()
        .find(|existing| source_selector_base(existing) == selector_base)
    {
        let existing_base = source_selector_base(existing).to_string();
        if selector != selector_base && existing.as_str() == existing_base {
            *existing = selector.to_string();
        }
    } else {
        selectors.push(selector.to_string());
    }
}

fn content_dump_decision(
    platform: &str,
    event: &str,
    action: &ToolAction,
    matches: Vec<DirectReadMatch<'_>>,
    subject_paths: Option<Vec<String>>,
    semantic_ast_patch_enabled: bool,
    recovery_prompt: &CompiledRecoveryPromptConfig,
) -> HookDecision {
    let routes = direct_read_routes(&matches);
    let providers = providers_from_matches(&matches);
    let message = source_access_recovery_message(
        platform,
        "bulk-source-dump",
        &providers,
        &routes,
        semantic_ast_patch_enabled,
        recovery_prompt,
    );
    let mut subject = subject_for_action(action);
    if let Some(paths) = subject_paths {
        subject.paths = paths;
    }
    deny_for_action(
        platform,
        event,
        ReasonKind::BulkSourceDump,
        action,
        direct_read_language_ids(&matches),
        subject,
        routes,
        message,
    )
}

fn direct_read_decision(
    _registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    semantic_ast_patch_enabled: bool,
    recovery_prompt: &CompiledRecoveryPromptConfig,
    matches: Vec<DirectReadMatch<'_>>,
) -> Option<HookDecision> {
    if matches.is_empty() {
        return None;
    }
    let routes = direct_read_routes(&matches);
    let message = direct_read_decision_message(
        platform,
        &matches,
        &routes,
        semantic_ast_patch_enabled,
        recovery_prompt,
    );
    Some(deny_for_action(
        platform,
        event,
        ReasonKind::DirectSourceRead,
        action,
        direct_read_language_ids(&matches),
        subject_for_action(action),
        routes,
        message,
    ))
}

type DirectReadMatch<'provider> = SourceSelectorMatch<'provider>;

fn collect_direct_read_matches<'provider, I, S>(
    registry: &'provider HookRuntime,
    paths: I,
) -> Vec<DirectReadMatch<'provider>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    collect_source_selector_matches(registry, paths, |provider| {
        provider.policy.blocks_direct_source_read()
    })
}

fn collect_content_dump_matches<'provider, I, S>(
    registry: &'provider HookRuntime,
    paths: I,
) -> Vec<DirectReadMatch<'provider>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    collect_source_selector_matches(registry, paths, |provider| {
        provider.policy.blocks_bulk_source_dump()
    })
}

pub(super) fn direct_read_routes(matches: &[DirectReadMatch<'_>]) -> Vec<DecisionRoute> {
    matches
        .iter()
        .map(|matched| direct_read_route(matched.provider, &matched.route_selector, matched.kind))
        .collect()
}

pub(super) fn direct_read_language_ids(matches: &[DirectReadMatch<'_>]) -> Vec<String> {
    matches
        .iter()
        .map(|matched| matched.provider.language_id.clone())
        .collect()
}

fn direct_read_decision_message(
    platform: &str,
    matches: &[DirectReadMatch<'_>],
    routes: &[DecisionRoute],
    semantic_ast_patch_enabled: bool,
    recovery_prompt: &CompiledRecoveryPromptConfig,
) -> String {
    let providers = providers_from_matches(matches);
    source_access_recovery_message(
        platform,
        "direct-source-read",
        &providers,
        routes,
        semantic_ast_patch_enabled,
        recovery_prompt,
    )
}

fn providers_from_matches<'a>(matches: &[DirectReadMatch<'a>]) -> Vec<&'a ActivatedProvider> {
    matches.iter().map(|matched| matched.provider).collect()
}

fn direct_read_route(
    provider: &ActivatedProvider,
    path: &str,
    selector_kind: SourceSelectorKind,
) -> DecisionRoute {
    match selector_kind {
        SourceSelectorKind::ExactPath => selector_query_route(provider, path),
        SourceSelectorKind::Pattern => {
            let route_context = provider.route_path_context(path);
            search_query_route_for_selector(
                provider,
                &route_context.selector,
                &route_context.project_root,
                &[],
            )
            .unwrap_or_else(|| {
                provider.route_from_template(
                    DecisionRouteKind::Prime,
                    &provider.routes.prime,
                    None,
                    None,
                )
            })
        }
    }
}

pub(super) fn classify_raw_search_command(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    tokens: &[String],
    semantic_ast_patch_enabled: bool,
    recovery_prompt: &CompiledRecoveryPromptConfig,
) -> Option<HookDecision> {
    if command_intent(tokens) != CommandIntent::RawSearch {
        return None;
    }
    let plan = raw_search_plan(registry, tokens)?;
    let raw_search_providers = plan
        .providers
        .into_iter()
        .filter(|provider| provider.policy.blocks_raw_source_search())
        .collect::<Vec<_>>();
    if raw_search_providers.is_empty() || contains_ingest_pipe(tokens, &raw_search_providers) {
        return None;
    }
    let terms = plan.terms;
    let routes: Vec<DecisionRoute> = raw_search_providers
        .iter()
        .map(|provider| {
            if terms.is_empty() {
                return raw_search_ingest_route(provider);
            }
            search_query_route(provider, &terms)
                .unwrap_or_else(|| raw_search_ingest_route(provider))
        })
        .collect();
    let message = source_access_recovery_message(
        platform,
        "raw-broad-search",
        &raw_search_providers,
        &routes,
        semantic_ast_patch_enabled,
        recovery_prompt,
    );
    Some(deny_for_action(
        platform,
        event,
        ReasonKind::RawBroadSearch,
        action,
        language_ids(&raw_search_providers),
        subject_for_action(action),
        routes,
        message,
    ))
}

fn raw_search_ingest_route(provider: &ActivatedProvider) -> DecisionRoute {
    provider.route_from_template(
        DecisionRouteKind::Ingest,
        &provider.routes.ingest,
        None,
        None,
    )
}

fn language_ids(providers: &[&ActivatedProvider]) -> Vec<String> {
    providers
        .iter()
        .map(|provider| provider.language_id.clone())
        .collect()
}
