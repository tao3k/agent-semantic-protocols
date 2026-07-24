//! Source-access, direct-read, and raw-search classifier routes.

use crate::hook_recovery_prompt::CompiledRecoveryPromptConfig;
use crate::{
    ActivatedProvider, DecisionRoute, DecisionRouteKind, HookDecision, HookRuntime,
    OperationIntent, ReasonKind, SourceSelectorKind, SourceSelectorMatch, ToolAction,
    collect_source_selector_matches, subject_for_action,
};

use super::decision::deny_for_action;
use super::recovery::source_access_recovery_message;

pub(super) fn classify_direct_read_action(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    agent_action: Option<&crate::tool_action::AgentAction>,
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

    let reads_directory = action.operation == OperationIntent::DirectoryRead
        || (action.operation == OperationIntent::DirectRead
            && action.paths.iter().any(|path| {
                let path = std::path::Path::new(path);
                if path.is_absolute() {
                    return path.is_dir();
                }
                std::path::Path::new(&registry.project_root)
                    .join(path)
                    .is_dir()
                    || std::env::current_dir()
                        .map(|current_dir| current_dir.join(path).is_dir())
                        .unwrap_or(false)
            }));

    if reads_directory {
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
            .map(|provider| source_access_ingest_route(provider))
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

    let inferred_execute_read = action.operation == OperationIntent::ShellCommand
        && agent_action.is_some_and(|agent_action| {
            agent_action.effect == crate::tool_action::AgentActionKind::Read
        });
    if action.operation != OperationIntent::DirectRead && !inferred_execute_read {
        return None;
    }

    let matches =
        collect_direct_source_read_matches(registry, action.paths.iter().map(String::as_str));
    if matches.is_empty() {
        return None;
    }

    if inferred_execute_read {
        Some(derive_execute_read_decision(
            platform,
            event,
            action,
            matches,
            semantic_ast_patch_enabled,
            recovery_prompt,
        ))
    } else {
        Some(direct_source_read_decision(
            platform,
            event,
            action,
            matches,
            semantic_ast_patch_enabled,
            recovery_prompt,
        ))
    }
}

type DirectReadMatch<'provider> = SourceSelectorMatch<'provider>;

fn direct_source_read_decision(
    platform: &str,
    event: &str,
    action: &ToolAction,
    matches: Vec<DirectReadMatch<'_>>,
    semantic_ast_patch_enabled: bool,
    recovery_prompt: &CompiledRecoveryPromptConfig,
) -> HookDecision {
    let routes = direct_read_routes(&matches);
    let providers = providers_from_matches(&matches);
    let message = source_access_recovery_message(
        platform,
        "direct-source-read",
        &providers,
        &routes,
        semantic_ast_patch_enabled,
        recovery_prompt,
    );
    deny_for_action(
        platform,
        event,
        ReasonKind::DirectSourceRead,
        action,
        direct_read_language_ids(&matches),
        subject_for_action(action),
        routes,
        message,
    )
}

fn derive_execute_read_decision(
    platform: &str,
    event: &str,
    action: &ToolAction,
    matches: Vec<DirectReadMatch<'_>>,
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
    deny_for_action(
        platform,
        event,
        ReasonKind::BulkSourceDump,
        action,
        direct_read_language_ids(&matches),
        subject_for_action(action),
        routes,
        message,
    )
}

fn collect_direct_source_read_matches<'provider, I, S>(
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

fn providers_from_matches<'a>(matches: &[DirectReadMatch<'a>]) -> Vec<&'a ActivatedProvider> {
    matches.iter().map(|matched| matched.provider).collect()
}

fn direct_read_route(
    provider: &ActivatedProvider,
    path: &str,
    selector_kind: SourceSelectorKind,
) -> DecisionRoute {
    match selector_kind {
        SourceSelectorKind::ExactPath => provider.route_from_template(
            DecisionRouteKind::Owner,
            &provider.routes.owner,
            Some(path),
            Some(path),
        ),
        SourceSelectorKind::Pattern => provider.route_from_template(
            DecisionRouteKind::Lexical,
            &provider.routes.lexical,
            Some(path),
            Some(path),
        ),
    }
}

fn source_access_ingest_route(provider: &ActivatedProvider) -> DecisionRoute {
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
