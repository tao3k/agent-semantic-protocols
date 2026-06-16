//! Action-frontier compiler for ASP-owned search pipe output.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::search_pipe_action_frontier::{ActionNode, ActionRoute, render_action_rows};
use super::search_pipe_action_model::PipeAction;
use super::search_pipe_model::Candidate;
use super::search_pipe_owner_roles::{
    suppress_low_cohesion_secondary_owner, suppress_low_cohesion_weak_axis_owner,
};
use super::search_pipe_quality::SearchPipeQuality;
use super::search_pipe_seed_decision::SeedActionIntent;
use super::search_query_wrapper_model::FdQueryPreview;

#[derive(Clone, Copy)]
pub(super) struct SearchPipeActionRequest<'a> {
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) scopes: &'a [PathBuf],
    pub(super) quality: &'a SearchPipeQuality,
    pub(super) candidates: &'a [Candidate],
    pub(super) ranked_compact: Option<&'a str>,
    pub(super) selector_actions: &'a [PipeAction],
    pub(super) fd_preview: Option<&'a FdQueryPreview>,
    pub(super) seed_action_intents: &'a [SeedActionIntent],
    pub(super) read_memory_selectors: &'a [String],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DelegationHint {
    target_actions: Vec<String>,
}

impl DelegationHint {
    fn render_line(&self) -> String {
        format!(
            "subagentHint=profile=asp-explorer mode=resident instances=single reuse=send_input spawn=if-missing forkContext=false branchPrompt=reasoning-tree stateOwner=parent fanin=receipt iterative=true decision=advisory runtimeOwner=agent-client modelClass=cheap readOnly=true noCode=true targetActions={} maxCommands=8 maxTurns=1 receipt=asp-search-subagent(role,action,evidence,missing,next,risk) reason=query-selector-low-confidence",
            self.target_actions.join(",")
        )
    }
}

pub(super) fn render_action_frontier(request: SearchPipeActionRequest<'_>) -> String {
    let scope_arg = display_scope_args(request.project_root, request.locator_root, request.scopes);
    let command_handles = command_handles(&request);
    let tree_sitter_handles = tree_sitter_handles(request.quality, request.ranked_compact);
    let actions = action_nodes(&request, &scope_arg);
    let mut rendered = String::new();
    rendered.push_str(&format!("commandHandles={command_handles}\n"));
    rendered.push_str(&format!("treeSitterHandles={tree_sitter_handles}\n"));
    if let Some(preview) = request.fd_preview {
        rendered.push_str(&preview.render_line());
        rendered.push('\n');
    }
    rendered.push_str(&render_action_rows(&actions));
    if actions.is_empty() {
        return rendered;
    }
    for hint in delegation_hints(request.quality, &actions) {
        rendered.push_str(&hint.render_line());
        rendered.push('\n');
    }
    if !request.quality.allow_query_selector {
        rendered.push_str("reason=query-selector-low-confidence,owner-seed-base-required\n");
        rendered.push_str(
            "llmHint=after-fd-query-combine-owner-candidates-with-declaration-names-before-rg-query\n",
        );
    }
    rendered
}

pub(super) fn sanitize_evidence_line(line: &str) -> String {
    line.split(';')
        .map(sanitize_evidence_segment)
        .collect::<Vec<_>>()
        .join(";")
}

fn sanitize_evidence_segment(segment: &str) -> String {
    if segment.contains("=hot:") || segment.starts_with("H=hot:") {
        return segment.replace("!code", "!hot");
    }
    if segment.contains("=field:")
        || segment.contains("=type:")
        || segment.contains("=collection:")
        || segment.starts_with("F=field:")
        || segment.starts_with("Y=type:")
        || segment.starts_with("C=collection:")
    {
        return segment.replace("!code", "!evidence");
    }
    segment.to_string()
}

fn command_handles(request: &SearchPipeActionRequest<'_>) -> String {
    let fd = request.quality.fd_query.as_deref().unwrap_or("-");
    let rg = rg_query(request.quality, request.ranked_compact).unwrap_or_else(|| "-".to_string());
    let owner =
        owner_items_handle(request.quality, request.candidates).unwrap_or_else(|| "-".to_string());
    format!("fdQuery={fd};rgQuery={rg};ownerItems={owner}")
}

fn tree_sitter_handles(quality: &SearchPipeQuality, compact: Option<&str>) -> String {
    let fields = compact_symbols(compact, "field")
        .into_iter()
        .filter(|symbol| usable_query_term(symbol))
        .collect::<Vec<_>>();
    let mut handles = Vec::new();
    if !fields.is_empty() {
        handles.push(format!("interface-fields:{}", fields.join("|")));
    }
    if !quality.owner_seed_terms.is_empty() {
        handles.push(format!(
            "exported-declarations:{}",
            quality.owner_seed_terms.join("|")
        ));
    }
    if handles.is_empty() {
        "-".to_string()
    } else {
        handles.join(";")
    }
}

fn delegation_hints(quality: &SearchPipeQuality, actions: &[ActionNode]) -> Vec<DelegationHint> {
    if quality.allow_query_selector {
        return Vec::new();
    }
    let target_actions = actions
        .iter()
        .filter(|action| {
            matches!(
                action.kind.as_str(),
                "fd-query" | "rg-query" | "rg-query-set" | "owner-items" | "treesitter-query"
            )
        })
        .map(|action| format!("{}.{}", action.id, action.kind))
        .collect::<Vec<_>>();
    if target_actions.is_empty() {
        return Vec::new();
    }
    vec![DelegationHint { target_actions }]
}

fn action_nodes(request: &SearchPipeActionRequest<'_>, scope_arg: &str) -> Vec<ActionNode> {
    let mut actions = Vec::new();
    let prefer_owner_scope_first = request.quality.package_cohesion == "low"
        && (request.quality.fd_query.is_none() || request.fd_preview.is_some());
    let mut pushed_preferred_owner_items = false;
    if prefer_owner_scope_first
        && let Some(handle) = preferred_owner_items_handle(request)
        && let Some((owner, query)) = handle.split_once(':')
    {
        actions.push(owner_items_action(
            request.language_id,
            scope_arg,
            owner,
            query,
        ));
        pushed_preferred_owner_items = true;
    }
    if let Some(queries) = query_pack_queries(request) {
        actions.push(ActionNode {
            id: String::new(),
            kind: "rg-query-set".to_string(),
            suffix: "query-pack-refine".to_string(),
            route: ActionRoute::RgQuerySet {
                queries,
                scope: scope_arg.to_string(),
                command_scope: scope_arg.to_string(),
            },
        });
    }
    if request.quality.allow_query_selector
        && let Some(action) = request
            .selector_actions
            .iter()
            .find(|action| !selector_seen_in_read_memory(request, action))
    {
        actions.push(query_code_action(request, action));
    }
    if let Some(handle) = preview_owner_items_handle(request.quality, request.fd_preview)
        && let Some((owner, query)) = handle.split_once(':')
        && !pushed_preferred_owner_items
    {
        actions.push(owner_items_action(
            request.language_id,
            scope_arg,
            owner,
            query,
        ));
    }
    if request.fd_preview.is_none()
        && let Some(fd_query) = &request.quality.fd_query
    {
        actions.push(ActionNode {
            id: String::new(),
            kind: "fd-query".to_string(),
            suffix: "finder-owner".to_string(),
            route: ActionRoute::FdQuery {
                query: fd_query.to_string(),
                scope: scope_arg.to_string(),
                command_scope: None,
            },
        });
    }
    if let Some(query) = rg_query(request.quality, request.ranked_compact) {
        actions.push(ActionNode {
            id: String::new(),
            kind: "rg-query".to_string(),
            suffix: "finder-content".to_string(),
            route: ActionRoute::RgQuery {
                query,
                scope: scope_arg.to_string(),
                command_scope: None,
            },
        });
    }
    if request.fd_preview.is_none()
        && let Some(handle) = owner_items_handle(request.quality, request.candidates)
        && let Some((owner, query)) = handle.split_once(':')
        && !pushed_preferred_owner_items
    {
        actions.push(owner_items_action(
            request.language_id,
            scope_arg,
            owner,
            query,
        ));
    }
    if let Some(handle) = tree_sitter_action_handle(request.quality, request.ranked_compact) {
        let recipe = handle_field(&handle, "recipe").map(str::to_string);
        let names = handle_field(&handle, "names").map(|names| {
            names
                .split('|')
                .filter(|name| usable_query_term(name))
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

fn preferred_owner_items_handle(request: &SearchPipeActionRequest<'_>) -> Option<String> {
    preview_owner_items_handle(request.quality, request.fd_preview).or_else(|| {
        (!suppress_low_cohesion_secondary_owner(request.quality, request.fd_preview))
            .then(|| owner_items_handle(request.quality, request.candidates))
            .flatten()
    })
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

fn query_pack_queries(request: &SearchPipeActionRequest<'_>) -> Option<Vec<String>> {
    let has_split = request
        .seed_action_intents
        .contains(&SeedActionIntent::SplitQueryPack);
    let has_narrow_owner_scope = request
        .seed_action_intents
        .contains(&SeedActionIntent::NarrowOwnerScope);
    if !has_split || !has_narrow_owner_scope {
        return None;
    }
    if let Some(queries) = query_pack_hint_queries(request.quality) {
        return Some(queries);
    }
    let mut terms = Vec::new();
    terms.extend(request.quality.context_terms.iter().cloned());
    terms.extend(request.quality.owner_seed_terms.iter().cloned());
    terms.extend(request.quality.concept_terms.iter().cloned());
    if terms.len() < 4 {
        return None;
    }
    let split_at = terms.len().div_ceil(2);
    Some(vec![
        terms[..split_at].join(" "),
        terms[split_at..].join(" "),
    ])
}

fn query_pack_hint_queries(quality: &SearchPipeQuality) -> Option<Vec<String>> {
    let queries = quality
        .next_query_pack_hint
        .as_deref()?
        .split('|')
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    (queries.len() > 1).then_some(queries)
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
    if action.source_alias.starts_with('I')
        && let Some((path, start, end)) = selector_parts(&action.selector)
        && start == end
    {
        let context_start = start.saturating_sub(8).max(1);
        let context_end = end + 12;
        return format!("{path}:{context_start}:{context_end}");
    }
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

fn rg_query(quality: &SearchPipeQuality, compact: Option<&str>) -> Option<String> {
    let mut terms = Vec::new();
    terms.extend(quality.concept_terms.iter().cloned());
    terms.extend(quality.owner_seed_terms.iter().cloned());
    terms.extend(
        compact_symbols(compact, "field")
            .into_iter()
            .chain(compact_symbols(compact, "hot"))
            .filter(|symbol| usable_query_term(symbol) && !weak_natural_action_term(symbol)),
    );
    unique_terms_without_weak_natural(terms, 8).map(|terms| terms.join("|"))
}

fn owner_items_handle(quality: &SearchPipeQuality, candidates: &[Candidate]) -> Option<String> {
    let owner = quality.best_owner.as_ref()?.owner.as_str();
    let mut query_terms = Vec::new();
    query_terms.extend(quality.concept_terms.iter().cloned());
    query_terms.extend(quality.owner_seed_terms.iter().cloned());
    query_terms.extend(
        candidates
            .iter()
            .filter(|candidate| candidate.path == owner)
            .map(|candidate| candidate.symbol.clone()),
    );
    let query_terms = unique_terms_without_weak_natural(query_terms, 6)?;
    if suppress_low_cohesion_weak_axis_owner(quality, &query_terms) {
        return None;
    }
    let query = query_terms.join("|");
    Some(format!("{owner}:{query}"))
}

fn preview_owner_items_handle(
    quality: &SearchPipeQuality,
    preview: Option<&FdQueryPreview>,
) -> Option<String> {
    let preview = preview?;
    if quality.package_cohesion == "low" && strong_owner_seed_count(quality) < 2 {
        return None;
    }
    let owner = quality
        .best_owner
        .as_ref()
        .map(|coverage| coverage.owner.as_str())
        .filter(|owner| {
            preview
                .owner_candidates
                .iter()
                .any(|candidate| candidate == owner)
        })
        .or_else(|| preview.owner_candidates.first().map(String::as_str))?;
    let mut query_terms = Vec::new();
    query_terms.extend(quality.concept_terms.iter().cloned());
    query_terms.extend(quality.owner_seed_terms.iter().cloned());
    let query = unique_terms_without_weak_natural(query_terms, 6)?.join("|");
    Some(format!("{owner}:{query}"))
}

fn strong_owner_seed_count(quality: &SearchPipeQuality) -> usize {
    quality
        .owner_seed_terms
        .iter()
        .filter(|term| {
            quality
                .strong_matched
                .iter()
                .any(|matched| matched == *term)
        })
        .count()
}

fn tree_sitter_action_handle(quality: &SearchPipeQuality, compact: Option<&str>) -> Option<String> {
    let fields = compact_symbols(compact, "field")
        .into_iter()
        .filter(|symbol| usable_query_term(symbol))
        .collect::<Vec<_>>();
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

fn usable_query_term(term: &str) -> bool {
    !term.starts_with('_')
        && !term.starts_with('[')
        && term
            .chars()
            .all(|ch| ch == '.' || ch == '_' || ch.is_ascii_alphanumeric())
}

fn unique_terms(terms: Vec<String>, limit: usize) -> Option<Vec<String>> {
    let mut seen = BTreeSet::new();
    let result = terms
        .into_iter()
        .filter(|term| usable_query_term(term))
        .filter(|term| seen.insert(term.clone()))
        .take(limit)
        .collect::<Vec<_>>();
    (!result.is_empty()).then_some(result)
}

fn unique_terms_without_weak_natural(terms: Vec<String>, limit: usize) -> Option<Vec<String>> {
    unique_terms(
        terms
            .into_iter()
            .filter(|term| !weak_natural_action_term(term))
            .collect(),
        limit,
    )
}

fn weak_natural_action_term(term: &str) -> bool {
    matches!(
        term.to_ascii_lowercase().as_str(),
        "through"
            | "smoke"
            | "dev"
            | "dependency"
            | "dependencies"
            | "weak"
            | "natural"
            | "term"
            | "terms"
    )
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
