//! Typed search-pipe action frontier facts and display materialization.

use serde_json::{Value, json};

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
    },
    RgQuery {
        query: String,
        scope: String,
    },
    OwnerItems {
        language_id: String,
        owner: String,
        query: String,
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
                format!("selector={selector},owner={owner},symbol={symbol}")
            }
            ActionRoute::FdQuery { query, scope } | ActionRoute::RgQuery { query, scope } => {
                format!("query={query},scope={scope}")
            }
            ActionRoute::OwnerItems { owner, query, .. } => {
                format!("owner={owner},query={query}")
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
            } => Some(format!(
                "asp {language_id} query --selector {} --workspace {workspace} --code",
                shell_arg(selector)
            )),
            ActionRoute::FdQuery { query, scope } => {
                Some(format!("asp fd -query {} {scope}", shell_arg(query)))
            }
            ActionRoute::RgQuery { query, scope } => {
                Some(format!("asp rg -query {} {scope}", shell_arg(query)))
            }
            ActionRoute::OwnerItems {
                language_id,
                owner,
                query,
                scope,
            } => Some(format!(
                "asp {language_id} search owner {} items --query {} --view seeds {scope}",
                shell_arg(owner),
                shell_arg(query),
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
                    "asp {language_id} query --treesitter-query {} {scope}",
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
                fields.insert("ownerPath".to_string(), json!(owner));
                fields.insert("symbol".to_string(), json!(symbol));
                fields.insert("workspace".to_string(), json!(workspace));
                ("query", selector.as_str(), "selector")
            }
            ActionRoute::FdQuery { query, scope } => {
                fields.insert("query".to_string(), json!(query));
                fields.insert("scope".to_string(), json!(scope));
                ("fd", query.as_str(), "query")
            }
            ActionRoute::RgQuery { query, scope } => {
                fields.insert("query".to_string(), json!(query));
                fields.insert("scope".to_string(), json!(scope));
                ("rg", query.as_str(), "query")
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
