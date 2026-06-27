//! Typed search-pipe action frontier facts and display materialization.

use serde_json::{Value, json};

use super::search_pipe_projection::{query_projection_flag, query_projection_kind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ActionNode {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) suffix: String,
    pub(super) route: ActionRoute,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ActionRoute {
    QueryCode {
        language_id: String,
        selector: String,
        owner: String,
        symbol: String,
        workspace: String,
    },
    FdQuery {
        query: String,
        scope: String,
        command_scope: Option<String>,
    },
    RgQuery {
        query: String,
        scope: String,
        command_scope: Option<String>,
    },
    RgQuerySet {
        queries: Vec<String>,
        scope: String,
        command_scope: String,
    },
    OwnerItems {
        language_id: String,
        owner: String,
        query: String,
        scope: String,
    },
    OwnerItemsHint {
        owner: String,
    },
    DependencySearch {
        language_id: String,
        dependency: String,
        scope: String,
    },
    TreeSitterQuery {
        language_id: String,
        recipe: String,
        names: Vec<String>,
        scope: String,
    },
}

impl ActionNode {
    pub(super) fn render_body(&self) -> String {
        match &self.route {
            ActionRoute::QueryCode {
                selector,
                owner,
                symbol,
                ..
            } => {
                format!(
                    "sourceLocatorHint={selector},owner={owner},symbol={symbol},codePolicy=requires-exact-code"
                )
            }
            ActionRoute::FdQuery { query, scope, .. }
            | ActionRoute::RgQuery { query, scope, .. } => format!("query={query},scope={scope}"),
            ActionRoute::RgQuerySet { queries, scope, .. } => {
                format!(
                    "queryClauses={},scope={scope}",
                    query_clauses_display(queries)
                )
            }
            ActionRoute::OwnerItems { owner, query, .. } => {
                format!("owner={owner},query={query}")
            }
            ActionRoute::OwnerItemsHint { owner } => format!("owner={owner}"),
            ActionRoute::DependencySearch {
                dependency, scope, ..
            } => {
                format!("dependency={dependency},scope={scope}")
            }
            ActionRoute::TreeSitterQuery { recipe, names, .. } => {
                format!("recipe={recipe},names={}", names.join("|"))
            }
        }
    }

    pub(super) fn materialized_command(&self) -> Option<String> {
        match &self.route {
            ActionRoute::QueryCode {
                language_id,
                selector,
                workspace,
                ..
            } => {
                let projection_flag = query_projection_flag(language_id);
                Some(format!(
                    "asp {language_id} query --selector {} --workspace {workspace} {projection_flag}",
                    shell_arg(selector)
                ))
            }
            ActionRoute::FdQuery {
                query,
                scope,
                command_scope,
            } => Some(format!(
                "asp fd -query {} --workspace {}",
                shell_arg(query),
                command_scope.as_deref().unwrap_or(scope)
            )),
            ActionRoute::RgQuery {
                query,
                scope,
                command_scope,
            } => Some(format!(
                "asp rg -query {} --workspace {}",
                shell_arg(query),
                command_scope.as_deref().unwrap_or(scope)
            )),
            ActionRoute::RgQuerySet {
                queries,
                command_scope,
                ..
            } => Some(format!(
                "asp rg {} --workspace {command_scope}",
                repeated_query_args(queries)
            )),
            ActionRoute::OwnerItems {
                language_id,
                owner,
                query,
                scope,
            } => Some(format!(
                "asp {language_id} search owner {} items --query {} --workspace {scope} --view seeds",
                shell_arg(owner),
                shell_arg(query),
            )),
            ActionRoute::OwnerItemsHint { .. } => None,
            ActionRoute::DependencySearch {
                language_id,
                dependency,
                scope,
            } => Some(format!(
                "asp {language_id} search deps {} --workspace {scope} --view seeds",
                shell_arg(dependency)
            )),
            ActionRoute::TreeSitterQuery {
                language_id,
                recipe,
                names,
                scope,
            } => {
                let borrowed_names = names.iter().map(String::as_str).collect::<Vec<_>>();
                let query = tree_sitter_query_pattern(language_id, recipe, &borrowed_names)?;
                Some(format!(
                    "asp {language_id} query --treesitter-query {} --workspace {scope}",
                    shell_arg(&query)
                ))
            }
        }
    }

    pub(super) fn as_json(&self) -> Value {
        let mut fields = serde_json::Map::new();
        let (capability_id, target, target_role) = match &self.route {
            ActionRoute::QueryCode {
                language_id,
                selector,
                owner,
                symbol,
                workspace,
            } => {
                fields.insert("languageId".to_string(), json!(language_id));
                fields.insert("selector".to_string(), json!(selector));
                if is_source_locator(selector) {
                    fields.insert("sourceLocatorHint".to_string(), json!(selector));
                    fields.insert("codePolicy".to_string(), json!("requires-exact-code"));
                    fields.insert("requiresExact".to_string(), json!(true));
                }
                fields.insert("ownerPath".to_string(), json!(owner));
                fields.insert("symbol".to_string(), json!(symbol));
                fields.insert("workspace".to_string(), json!(workspace));
                fields.insert(
                    "projection".to_string(),
                    json!(query_projection_kind(language_id)),
                );
                ("query", owner.as_str(), "owner")
            }
            ActionRoute::FdQuery { query, scope, .. } => {
                fields.insert("query".to_string(), json!(query));
                fields.insert("scope".to_string(), json!(scope));
                ("fd", query.as_str(), "query")
            }
            ActionRoute::RgQuery { query, scope, .. } => {
                fields.insert("query".to_string(), json!(query));
                fields.insert("scope".to_string(), json!(scope));
                ("rg", query.as_str(), "query")
            }
            ActionRoute::RgQuerySet { queries, scope, .. } => {
                fields.insert("queryClauses".to_string(), json!(queries));
                fields.insert("scope".to_string(), json!(scope));
                ("rg", scope.as_str(), "query-set")
            }
            ActionRoute::OwnerItems {
                language_id,
                owner,
                query,
                scope,
            } => {
                fields.insert("languageId".to_string(), json!(language_id));
                fields.insert("ownerPath".to_string(), json!(owner));
                fields.insert("query".to_string(), json!(query));
                fields.insert("scope".to_string(), json!(scope));
                ("owner-items", owner.as_str(), "owner")
            }
            ActionRoute::OwnerItemsHint { owner } => {
                fields.insert("ownerPath".to_string(), json!(owner));
                ("owner-items", owner.as_str(), "owner")
            }
            ActionRoute::DependencySearch {
                language_id,
                dependency,
                scope,
            } => {
                fields.insert("languageId".to_string(), json!(language_id));
                fields.insert("dependency".to_string(), json!(dependency));
                fields.insert("scope".to_string(), json!(scope));
                ("search-deps", dependency.as_str(), "dependency")
            }
            ActionRoute::TreeSitterQuery {
                language_id,
                recipe,
                names,
                scope,
            } => {
                fields.insert("languageId".to_string(), json!(language_id));
                fields.insert("recipe".to_string(), json!(recipe));
                fields.insert("names".to_string(), json!(names));
                fields.insert("scope".to_string(), json!(scope));
                ("treesitter-query", recipe.as_str(), "syntax-recipe")
            }
        };
        json!({
            "id": self.id,
            "kind": self.kind,
            "capabilityId": capability_id,
            "target": target,
            "targetRole": target_role,
            "fields": fields,
        })
    }
}

pub(super) fn render_action_rows(actions: &[ActionNode]) -> String {
    let mut rendered = String::new();
    if actions.is_empty() {
        rendered.push_str("actionRank=-\n");
        rendered.push_str("actionFrontier=-\n");
        rendered.push_str("recommendedNext=-\n");
        return rendered;
    }
    rendered.push_str(&render_route_graph_rows(actions));
    rendered.push_str(&format!(
        "actionRank={}\n",
        actions
            .iter()
            .map(|action| action.id.as_str())
            .collect::<Vec<_>>()
            .join(",")
    ));
    for action in actions {
        rendered.push_str(&format!(
            "{}={}({})!{}\n",
            action.id,
            action.kind,
            action.render_body(),
            action.suffix
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
    if let Some(command) = first.materialized_command() {
        rendered.push_str(&format!("nextCommand={command}\n"));
    }
    rendered
}

fn render_route_graph_rows(actions: &[ActionNode]) -> String {
    let first = actions.first().expect("non-empty actions");
    let (evidence, chosen, reason, avoid) = route_graph_metadata(&first.route);
    let frontier = actions
        .iter()
        .map(|action| format!("{}.{}", action.id, action.kind))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "[route-graph] profile=asp-search-routing evidence={evidence} chosen={chosen} reason=\"{reason}\" routeFrontier={frontier} routeAvoid={avoid}\n"
    )
}

fn route_graph_metadata(
    route: &ActionRoute,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match route {
        ActionRoute::QueryCode { .. } => (
            "known-selector+known-owner+symbol",
            "KNOWN_SELECTOR",
            "exact selector and owner/symbol evidence are available",
            "search-prime|line-range-selector|direct-source-read",
        ),
        ActionRoute::OwnerItems { .. } | ActionRoute::OwnerItemsHint { .. } => (
            "known-owner",
            "KNOWN_OWNER",
            "owner evidence is available; inspect owner items before broader search",
            "search-prime|direct-source-read",
        ),
        ActionRoute::DependencySearch { .. } => (
            "known-dependency",
            "KNOWN_DEPENDENCY",
            "dependency evidence is available; inspect topology/import usage",
            "workspace-prime|direct-source-read",
        ),
        ActionRoute::TreeSitterQuery { .. } => (
            "known-selector",
            "KNOWN_SELECTOR",
            "structural query evidence is available",
            "search-prime|line-range-selector",
        ),
        ActionRoute::FdQuery { .. }
        | ActionRoute::RgQuery { .. }
        | ActionRoute::RgQuerySet { .. } => (
            "broad-query",
            "BROAD_QUERY",
            "query has no stable owner/selector anchor; refine finder evidence",
            "repeat-search-pipe|manual-window-scan|direct-source-read",
        ),
    }
}

fn query_clauses_display(queries: &[String]) -> String {
    queries
        .iter()
        .enumerate()
        .map(|(index, query)| format!("C{}={}", index + 1, shell_arg(query)))
        .collect::<Vec<_>>()
        .join(";")
}

fn repeated_query_args(queries: &[String]) -> String {
    queries
        .iter()
        .map(|query| format!("-query {}", shell_arg(query)))
        .collect::<Vec<_>>()
        .join(" ")
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

fn is_source_locator(value: &str) -> bool {
    let Some((_, range)) = value.split_once(':') else {
        return false;
    };
    let mut parts = range.split([':', '-']);
    let Some(start) = parts.next() else {
        return false;
    };
    let Some(end) = parts.next() else {
        return false;
    };
    !start.is_empty()
        && !end.is_empty()
        && start.chars().all(|character| character.is_ascii_digit())
        && end.chars().all(|character| character.is_ascii_digit())
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
