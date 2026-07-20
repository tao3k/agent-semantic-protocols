//! Action-frontier compiler for ASP-owned search pipe output.

use std::path::{Path, PathBuf};

use super::search_pipe_action_frontier::{ActionNode, ActionRoute, render_next_command_line};
use super::search_pipe_action_model::PipeAction;
use super::search_pipe_quality_model::SearchPipeQuality;

#[derive(Clone, Copy)]
pub(super) struct SearchPipeActionRequest<'a> {
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) scopes: &'a [PathBuf],
    pub(super) quality: &'a SearchPipeQuality,
    pub(super) ranked_compact: Option<&'a str>,
    pub(super) selector_actions: &'a [PipeAction],
    pub(super) read_memory_selectors: &'a [String],
    pub(super) dependency_action_targets: &'a [String],
}

pub(super) fn render_action_next_command(request: SearchPipeActionRequest<'_>) -> String {
    let scope_arg = display_scope_args(request.project_root, request.locator_root, request.scopes);
    let actions = action_nodes(&request, &scope_arg);
    render_next_command_line(&actions)
}

fn action_nodes(request: &SearchPipeActionRequest<'_>, scope_arg: &str) -> Vec<ActionNode> {
    let mut actions = Vec::new();
    if let Some(dependency) = request.dependency_action_targets.first() {
        actions.push(dependency_search_action(
            request.language_id,
            dependency,
            scope_arg,
        ));
    }
    let mut pushed_preferred_owner_items = false;
    if request.quality.package_cohesion == "low"
        && let Some(action) = owner_items_action_from_quality(request, scope_arg)
    {
        actions.push(action);
        pushed_preferred_owner_items = true;
    }
    if request.quality.allow_query_selector
        && let Some(action) = request
            .selector_actions
            .iter()
            .find(|action| !selector_seen_in_read_memory(request, action))
    {
        actions.push(query_code_action(request, action));
    }
    if !pushed_preferred_owner_items
        && let Some(action) = owner_items_action_from_quality(request, scope_arg)
    {
        actions.push(action);
    }
    if let Some(handle) = tree_sitter_action_handle(request.quality, request.ranked_compact) {
        let recipe = handle_field(&handle, "recipe").map(str::to_string);
        let names = handle_field(&handle, "names").map(|names| {
            names
                .split('|')
                .filter(|name| !name.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        });
        let Some((recipe, names)) = recipe.zip(names).filter(|(_, names)| !names.is_empty()) else {
            return actions
                .into_iter()
                .enumerate()
                .map(|(index, mut action)| {
                    action.id = format!("A{}", index + 1);
                    action
                })
                .collect();
        };
        actions.push(ActionNode {
            id: String::new(),
            kind: "treesitter-query".to_string(),
            suffix: "syntax-locator".to_string(),
            route: ActionRoute::TreeSitterQuery {
                language_id: request.language_id.to_string(),
                recipe,
                names,
                scope: scope_arg.to_string(),
            },
        });
    }
    actions
        .into_iter()
        .enumerate()
        .map(|(index, mut action)| {
            action.id = format!("A{}", index + 1);
            action
        })
        .collect()
}

fn dependency_search_action(language_id: &str, dependency: &str, scope_arg: &str) -> ActionNode {
    ActionNode {
        id: String::new(),
        kind: "search-deps".to_string(),
        suffix: "dependency-topology".to_string(),
        route: ActionRoute::DependencySearch {
            language_id: language_id.to_string(),
            dependency: dependency.to_string(),
            scope: scope_arg.to_string(),
        },
    }
}

fn owner_items_action(language_id: &str, scope_arg: &str, owner: &str, query: &str) -> ActionNode {
    ActionNode {
        id: String::new(),
        kind: "owner-items".to_string(),
        suffix: "owner-items".to_string(),
        route: ActionRoute::OwnerItems {
            language_id: language_id.to_string(),
            owner: owner.to_string(),
            query: query.to_string(),
            scope: scope_arg.to_string(),
        },
    }
}

fn query_code_action(request: &SearchPipeActionRequest<'_>, action: &PipeAction) -> ActionNode {
    let selector = query_code_selector(action);
    let workspace_arg = action_root_arg(
        action,
        request.project_root,
        request.locator_root,
        request.scopes,
    );
    ActionNode {
        id: String::new(),
        kind: "query-code".to_string(),
        suffix: "terminal-code".to_string(),
        route: ActionRoute::QueryCode {
            language_id: request.language_id.to_string(),
            selector,
            owner: action.owner.clone(),
            symbol: action.symbol.clone(),
            workspace: workspace_arg,
        },
    }
}

fn query_code_selector(action: &PipeAction) -> String {
    action.selector.clone()
}

fn selector_seen_in_read_memory(
    request: &SearchPipeActionRequest<'_>,
    action: &PipeAction,
) -> bool {
    let query_selector = query_code_selector(action);
    request.read_memory_selectors.iter().any(|selector| {
        selector_matches_seen(selector, &action.selector)
            || selector_matches_seen(selector, &query_selector)
    })
}

fn selector_matches_seen(seen: &str, candidate: &str) -> bool {
    if seen == candidate {
        return true;
    }
    let Some((seen_path, seen_start, seen_end)) = selector_parts(seen) else {
        return false;
    };
    let Some((candidate_path, candidate_start, candidate_end)) = selector_parts(candidate) else {
        return false;
    };
    seen_path == candidate_path && seen_start <= candidate_start && candidate_end <= seen_end
}

fn owner_items_action_from_quality(
    request: &SearchPipeActionRequest<'_>,
    scope_arg: &str,
) -> Option<ActionNode> {
    let owner = request.quality.best_owner.as_ref()?.owner.as_str();
    let query = request
        .quality
        .owner_seed_terms
        .iter()
        .chain(request.quality.strong_matched.iter())
        .chain(request.quality.concept_terms.iter())
        .take(6)
        .cloned()
        .collect::<Vec<_>>()
        .join("|");
    (!query.is_empty()).then(|| owner_items_action(request.language_id, scope_arg, owner, &query))
}

fn tree_sitter_action_handle(quality: &SearchPipeQuality, compact: Option<&str>) -> Option<String> {
    let fields = compact_symbols(compact, "field");
    if !fields.is_empty() {
        return Some(format!(
            "recipe=interface-fields,names={}",
            fields.join("|")
        ));
    }
    if !quality.owner_seed_terms.is_empty() {
        return Some(format!(
            "recipe=exported-declarations,names={}",
            quality.owner_seed_terms.join("|")
        ));
    }
    None
}

fn handle_field<'a>(handle: &'a str, key: &str) -> Option<&'a str> {
    handle.split(',').find_map(|field| {
        let (field_key, value) = field.split_once('=')?;
        (field_key == key && !value.is_empty()).then_some(value)
    })
}

fn compact_symbols(compact: Option<&str>, kind: &str) -> Vec<String> {
    let Some(compact) = compact else {
        return Vec::new();
    };
    compact
        .lines()
        .flat_map(|line| line.split(';'))
        .filter(|segment| segment.contains(&format!("={kind}:")) || segment.starts_with(kind))
        .filter_map(node_symbol)
        .collect()
}

fn node_symbol(segment: &str) -> Option<String> {
    let start = segment.find('(')? + 1;
    let end = segment[start..].find(')')? + start;
    let symbol = segment[start..end].trim();
    (!symbol.is_empty()).then(|| symbol.to_string())
}

fn display_scope_args(project_root: &Path, locator_root: &Path, scopes: &[PathBuf]) -> String {
    if scopes.is_empty() {
        return display_project_root_arg(project_root);
    }
    scopes
        .iter()
        .map(|scope| display_scope_arg(project_root, locator_root, scope))
        .collect::<Vec<_>>()
        .join(" ")
}

fn action_root_arg(
    action: &PipeAction,
    project_root: &Path,
    locator_root: &Path,
    scopes: &[PathBuf],
) -> String {
    let Some(path) = selector_path(&action.selector) else {
        return display_project_root_arg(project_root);
    };
    let path = Path::new(path);
    if locator_root.join(path).exists() || path.is_absolute() {
        return display_project_root_arg(project_root);
    }
    for scope in scopes {
        let absolute = scope_absolute(project_root, scope);
        if absolute.join(path).exists() {
            return display_scope_arg(project_root, locator_root, scope);
        }
    }
    display_project_root_arg(project_root)
}

fn selector_path(selector: &str) -> Option<&str> {
    if let Some((_, rest)) = selector.split_once("://") {
        let (owner, _) = rest.split_once("#item/")?;
        return (!owner.is_empty()).then_some(owner);
    }
    let mut parts = selector.rsplitn(3, ':');
    let _end = parts.next()?;
    let _start = parts.next()?;
    let path = parts.next()?;
    (!path.is_empty()).then_some(path)
}

fn selector_parts(selector: &str) -> Option<(&str, usize, usize)> {
    let mut parts = selector.rsplitn(3, ':');
    let end = parts.next()?.parse::<usize>().ok()?;
    let start = parts.next()?.parse::<usize>().ok()?;
    let path = parts.next()?;
    (!path.is_empty() && start <= end).then_some((path, start, end))
}

fn display_project_root_arg(project_root: &Path) -> String {
    let Ok(cwd) = std::env::current_dir() else {
        return shell_arg(&slash_path(project_root));
    };
    if project_root == cwd {
        return ".".to_string();
    }
    let display = project_root
        .strip_prefix(&cwd)
        .map(slash_path)
        .unwrap_or_else(|_| slash_path(project_root));
    if display.is_empty() {
        ".".to_string()
    } else {
        shell_arg(&display)
    }
}

fn display_scope_arg(project_root: &Path, locator_root: &Path, scope: &Path) -> String {
    let absolute = scope_absolute(project_root, scope);
    if absolute == locator_root {
        return ".".to_string();
    }
    let display = absolute
        .strip_prefix(locator_root)
        .map(slash_path)
        .unwrap_or_else(|_| slash_path(&absolute));
    if display.is_empty() {
        ".".to_string()
    } else {
        shell_arg(&display)
    }
}

fn scope_absolute(project_root: &Path, scope: &Path) -> PathBuf {
    if scope.is_absolute() {
        scope.to_path_buf()
    } else {
        project_root.join(scope)
    }
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn shell_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':'))
    {
        value.to_string()
    } else {
        shell_quote(value)
    }
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
