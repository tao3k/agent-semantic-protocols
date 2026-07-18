//! Typed search-pipe action frontier facts and display materialization.

use super::search_pipe_projection::query_projection_flag;

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
    RgQuery {
        query: String,
        scope: String,
        command_scope: Option<String>,
    },
    OwnerItems {
        language_id: String,
        owner: String,
        query: String,
        scope: String,
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
            ActionRoute::RgQuery {
                query,
                scope,
                command_scope,
            } => Some(format!(
                "asp rg -query {} --workspace {}",
                shell_arg(query),
                command_scope.as_deref().unwrap_or(scope)
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
}

pub(super) fn render_next_command_line(actions: &[ActionNode]) -> String {
    actions
        .iter()
        .find_map(ActionNode::materialized_command)
        .map(|command| format!("nextCommand={command}\n"))
        .unwrap_or_else(|| "nextCommand=-\n".to_string())
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

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
