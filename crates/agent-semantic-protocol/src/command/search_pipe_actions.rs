//! Action-frontier compiler for ASP-owned search pipe output.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::search_pipe_action_model::PipeAction;
use super::search_pipe_model::Candidate;
use super::search_pipe_quality::SearchPipeQuality;
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActionNode {
    id: String,
    kind: String,
    body: String,
    suffix: String,
    command: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DelegationHint {
    target_actions: Vec<String>,
}

impl DelegationHint {
    fn render_line(&self) -> String {
        format!(
            "subagentHint=profile=asp-explorer decision=advisory runtimeOwner=agent-client modelClass=cheap readOnly=true noCode=true targetActions={} maxCommands=8 maxTurns=1 receipt=search-subagent(role,evidence,missing,next,risk) reason=query-selector-low-confidence",
            self.target_actions.join(",")
        )
    }

    pub(super) fn as_json(&self) -> Value {
        json!({
            "profile": "asp-explorer",
            "decision": "advisory",
            "runtimeOwner": "agent-client",
            "modelClass": "cheap",
            "readOnly": true,
            "noCode": true,
            "targetActions": self.target_actions.clone(),
            "maxCommands": 8,
            "maxTurns": 1,
            "reason": "query-selector-low-confidence",
            "receipt": {
                "kind": "search-subagent",
                "requiredFields": ["role", "evidence", "missing", "next", "risk"]
            }
        })
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
    if actions.is_empty() {
        rendered.push_str("actionRank=-\n");
        rendered.push_str("actionFrontier=-\n");
        rendered.push_str("recommendedNext=-\n");
        return rendered;
    }
    rendered.push_str(&format!(
        "actionRank={}\n",
        actions
            .iter()
            .map(|action| action.id.as_str())
            .collect::<Vec<_>>()
            .join(",")
    ));
    for action in &actions {
        rendered.push_str(&format!(
            "{}={}({})!{}\n",
            action.id, action.kind, action.body, action.suffix
        ));
    }
    rendered.push_str(&format!(
        "actionFrontier={}\n",
        actions
            .iter()
            .map(|action| format!("{}.{}", action.id, action.kind))
            .collect::<Vec<_>>()
            .join(",")
    ));
    let first = actions.first().expect("non-empty actions");
    rendered.push_str(&format!("recommendedNext={}.{}\n", first.id, first.kind));
    if let Some(command) = &first.command {
        rendered.push_str(&format!("nextCommand={command}\n"));
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

pub(super) fn delegation_hints_for_request(
    request: SearchPipeActionRequest<'_>,
) -> Vec<DelegationHint> {
    let scope_arg = display_scope_args(request.project_root, request.locator_root, request.scopes);
    let actions = action_nodes(&request, &scope_arg);
    delegation_hints(request.quality, &actions)
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
                "fd-query" | "rg-query" | "owner-items" | "treesitter-query"
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
    if request.quality.allow_query_selector
        && let Some(action) = request.selector_actions.first()
    {
        actions.push(query_code_action(request, action));
    }
    if let Some(handle) = preview_owner_items_handle(request.quality, request.fd_preview)
        && let Some((owner, query)) = handle.split_once(':')
    {
        actions.push(ActionNode {
            id: String::new(),
            kind: "owner-items".to_string(),
            body: format!("owner={owner},query={query}"),
            suffix: "owner-items".to_string(),
            command: Some(format!(
                "asp {language} search owner {owner} items --query {query} --view seeds {scope_arg}",
                language = request.language_id,
                owner = shell_arg(owner),
                query = shell_arg(query),
            )),
        });
    }
    if request.fd_preview.is_none()
        && let Some(fd_query) = &request.quality.fd_query
    {
        actions.push(ActionNode {
            id: String::new(),
            kind: "fd-query".to_string(),
            body: format!("query={fd_query},scope={scope_arg}"),
            suffix: "finder-owner".to_string(),
            command: Some(format!("asp fd -query {} {scope_arg}", shell_arg(fd_query))),
        });
    }
    if let Some(query) = rg_query(request.quality, request.ranked_compact) {
        actions.push(ActionNode {
            id: String::new(),
            kind: "rg-query".to_string(),
            body: format!("query={query},scope={scope_arg}"),
            suffix: "finder-content".to_string(),
            command: Some(format!("asp rg -query {} {scope_arg}", shell_arg(&query))),
        });
    }
    if request.fd_preview.is_none()
        && let Some(handle) = owner_items_handle(request.quality, request.candidates)
        && let Some((owner, query)) = handle.split_once(':')
    {
        actions.push(ActionNode {
            id: String::new(),
            kind: "owner-items".to_string(),
            body: format!("owner={owner},query={query}"),
            suffix: "owner-items".to_string(),
            command: Some(format!(
                "asp {language} search owner {owner} items --query {query} --view seeds {scope_arg}",
                language = request.language_id,
                owner = shell_arg(owner),
                query = shell_arg(query),
            )),
        });
    }
    if let Some(handle) = tree_sitter_action_handle(request.quality, request.ranked_compact) {
        let command = tree_sitter_action_command(request.language_id, &handle, scope_arg);
        actions.push(ActionNode {
            id: String::new(),
            kind: "treesitter-query".to_string(),
            body: handle,
            suffix: "syntax-locator".to_string(),
            command,
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

fn query_code_action(request: &SearchPipeActionRequest<'_>, action: &PipeAction) -> ActionNode {
    let selector = query_code_selector(action);
    let workspace_arg = action_root_arg(
        action,
        request.project_root,
        request.locator_root,
        request.scopes,
    );
    let command = format!(
        "asp {language} query --selector {selector} --workspace {workspace_arg} --code",
        language = request.language_id,
        selector = shell_arg(&selector),
    );
    ActionNode {
        id: String::new(),
        kind: "query-code".to_string(),
        body: format!(
            "selector={},owner={},symbol={}",
            selector, action.owner, action.symbol
        ),
        suffix: "terminal-code".to_string(),
        command: Some(command),
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
    let query = unique_terms_without_weak_natural(query_terms, 6)?.join("|");
    Some(format!("{owner}:{query}"))
}

fn preview_owner_items_handle(
    quality: &SearchPipeQuality,
    preview: Option<&FdQueryPreview>,
) -> Option<String> {
    let owner = preview?.owner_candidates.first()?;
    let mut query_terms = Vec::new();
    query_terms.extend(quality.concept_terms.iter().cloned());
    query_terms.extend(quality.owner_seed_terms.iter().cloned());
    let query = unique_terms_without_weak_natural(query_terms, 6)?.join("|");
    Some(format!("{owner}:{query}"))
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

fn tree_sitter_action_command(language_id: &str, handle: &str, scope_arg: &str) -> Option<String> {
    let recipe = handle_field(handle, "recipe")?;
    let names = handle_field(handle, "names")?
        .split('|')
        .filter(|name| usable_query_term(name))
        .collect::<Vec<_>>();
    let query = tree_sitter_query_pattern(language_id, recipe, &names)?;
    Some(format!(
        "asp {language_id} query --treesitter-query {} {scope_arg}",
        shell_arg(&query)
    ))
}

fn handle_field<'a>(handle: &'a str, key: &str) -> Option<&'a str> {
    handle.split(',').find_map(|field| {
        let (field_key, value) = field.split_once('=')?;
        (field_key == key && !value.is_empty()).then_some(value)
    })
}

fn tree_sitter_query_pattern(language_id: &str, recipe: &str, names: &[&str]) -> Option<String> {
    if names.is_empty() {
        return None;
    }
    let patterns = match (language_id, recipe) {
        ("rust", "interface-fields") => names
            .iter()
            .map(|name| {
                eq_name_pattern("field_declaration", "field_identifier", "field.name", name)
            })
            .collect::<Vec<_>>(),
        ("rust", "exported-declarations") => names
            .iter()
            .flat_map(|name| {
                [
                    eq_name_pattern("function_item", "identifier", "declaration.name", name),
                    eq_name_pattern("struct_item", "type_identifier", "declaration.name", name),
                    eq_name_pattern("enum_item", "type_identifier", "declaration.name", name),
                    eq_name_pattern("trait_item", "type_identifier", "declaration.name", name),
                    eq_name_pattern("type_item", "type_identifier", "declaration.name", name),
                ]
            })
            .collect::<Vec<_>>(),
        ("typescript", "interface-fields") => names
            .iter()
            .map(|name| {
                eq_name_pattern(
                    "property_signature",
                    "property_identifier",
                    "field.name",
                    name,
                )
            })
            .collect::<Vec<_>>(),
        ("typescript", "exported-declarations") => names
            .iter()
            .flat_map(|name| {
                [
                    eq_name_pattern(
                        "function_declaration",
                        "identifier",
                        "declaration.name",
                        name,
                    ),
                    eq_name_pattern(
                        "class_declaration",
                        "type_identifier",
                        "declaration.name",
                        name,
                    ),
                    eq_name_pattern(
                        "interface_declaration",
                        "type_identifier",
                        "declaration.name",
                        name,
                    ),
                    eq_name_pattern(
                        "type_alias_declaration",
                        "type_identifier",
                        "declaration.name",
                        name,
                    ),
                    eq_name_pattern(
                        "variable_declarator",
                        "identifier",
                        "declaration.name",
                        name,
                    ),
                ]
            })
            .collect::<Vec<_>>(),
        ("python", "exported-declarations") => names
            .iter()
            .flat_map(|name| {
                [
                    eq_name_pattern(
                        "function_definition",
                        "identifier",
                        "declaration.name",
                        name,
                    ),
                    eq_name_pattern("class_definition", "identifier", "declaration.name", name),
                ]
            })
            .collect::<Vec<_>>(),
        _ => return None,
    };
    Some(patterns.join(" "))
}

fn eq_name_pattern(node: &str, name_node: &str, capture: &str, name: &str) -> String {
    format!("({node} name: ({name_node}) @{capture} (#eq? @{capture} \"{name}\"))")
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
