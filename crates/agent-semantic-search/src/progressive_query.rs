// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Canonical Scheme grammar for the shared exact-selector Query Playbook.

use agent_semantic_scheme_syntax::SchemeDatum;
use serde::{Deserialize, Serialize};

pub const QUERY_PLAYBOOK_V1_GRAMMAR: &str = "(query [(workspace \"id\")] (producers (language ...) (documents ...)) (select (selectors \"producer://owner[#item/...]\" ...) [(projection source|callable-skeleton)] [(output human|json)]))";

/// Presentation selected for the public Query surface.
///
/// Query is an agent-facing source/syntax operation by default. The typed
/// Runtime packet is an implementation detail unless the Scheme expression
/// explicitly selects `(output json)`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueryOutputFormat {
    Human,
    Json,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProgressiveQueryRequest {
    Selector {
        #[serde(skip_serializing_if = "Option::is_none")]
        language: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        documents: Option<String>,
        selectors: Vec<String>,
        projection: String,
        output_format: QueryOutputFormat,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace: Option<String>,
    },
}

pub fn parse_progressive_query_args(args: &[String]) -> Result<ProgressiveQueryRequest, String> {
    if args.first().map(String::as_str) != Some("query")
        || args.get(1).map(String::as_str) != Some("playbook")
    {
        return Err("use `asp query playbook`".to_owned());
    }
    let Some(source) = args.get(2) else {
        return Err("Query Playbook requires one Scheme expression".to_owned());
    };
    if args.len() != 3 {
        return Err("Query Playbook accepts exactly one Scheme expression argument".to_owned());
    }
    let datums = agent_semantic_scheme_syntax::parse_scheme_datums(source)
        .map_err(|error| format!("invalid Query Playbook Scheme source: {error}"))?;
    let [SchemeDatum::List(root)] = datums.as_slice() else {
        return Err("expected one (query ...) expression".to_owned());
    };
    lower_query(root)
}

fn lower_query(root: &[SchemeDatum]) -> Result<ProgressiveQueryRequest, String> {
    if symbol(root.first()) != Some("query") {
        return Err("expected one (query ...) expression".to_owned());
    }
    let (workspace, producers_index, select_index) = match root {
        [_, SchemeDatum::List(target), _, _] if symbol(target.first()) == Some("workspace") => {
            let [_, SchemeDatum::String(workspace)] = target.as_slice() else {
                return Err(
                    "workspace requires exactly one registered WorkspaceId string".to_owned(),
                );
            };
            if !valid_registered_name(workspace) {
                return Err("workspace requires one registered WorkspaceId, not a path".to_owned());
            }
            (Some(workspace.clone()), 2, 3)
        }
        [_, _, _] => (None, 1, 2),
        _ => {
            return Err(format!("expected {QUERY_PLAYBOOK_V1_GRAMMAR}"));
        }
    };
    let SchemeDatum::List(producers) = &root[producers_index] else {
        return Err("expected a producers declaration".to_owned());
    };
    let (language, documents) = lower_producers(producers)?;
    let SchemeDatum::List(select) = &root[select_index] else {
        return Err("expected a select form".to_owned());
    };
    if symbol(select.first()) != Some("select") {
        return Err("expected a select form".to_owned());
    }
    let mut selectors = Vec::new();
    let mut projection = None;
    let mut output_format = None;
    for clause in &select[1..] {
        let SchemeDatum::List(values) = clause else {
            return Err("select clauses must be lists".to_owned());
        };
        match symbol(values.first()) {
            Some("selectors") => {
                if !selectors.is_empty() {
                    return Err("selectors may occur only once".to_owned());
                }
                selectors = values[1..]
                    .iter()
                    .map(|value| match value {
                        SchemeDatum::String(value) if !value.is_empty() => Ok(value.clone()),
                        _ => Err("selectors must be non-empty strings".to_owned()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
            }
            Some("projection") => {
                let [_, SchemeDatum::Symbol(value)] = values.as_slice() else {
                    return Err("projection requires exactly one symbol".to_owned());
                };
                if projection.replace(value.clone()).is_some() {
                    return Err("projection may occur only once".to_owned());
                }
            }
            Some("output") => {
                let [_, SchemeDatum::Symbol(value)] = values.as_slice() else {
                    return Err("output requires exactly one symbol".to_owned());
                };
                let value = match value.as_str() {
                    "human" => QueryOutputFormat::Human,
                    "json" => QueryOutputFormat::Json,
                    _ => return Err("query output must be human or json".to_owned()),
                };
                if output_format.replace(value).is_some() {
                    return Err("output may occur only once".to_owned());
                }
            }
            Some(operator) => {
                return Err(format!(
                    "Query Playbook does not support operator `{operator}`; select supports selectors, projection, and output; expected {QUERY_PLAYBOOK_V1_GRAMMAR}"
                ));
            }
            None => return Err("select clause must begin with an operator".to_owned()),
        }
    }
    if selectors.is_empty() {
        return Err("Query Playbook requires at least one selector".to_owned());
    }
    if selectors
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != selectors.len()
    {
        return Err("Query Playbook selectors must be unique".to_owned());
    }
    let projection = projection.unwrap_or_else(|| "source".to_owned());
    validate_projection(&projection)?;
    let selected = language
        .iter()
        .chain(documents.iter())
        .flat_map(|expression| expression.split('|'))
        .collect::<std::collections::BTreeSet<_>>();
    if selectors.iter().any(|selector| {
        selector
            .split_once("://")
            .is_none_or(|(producer, _)| !selected.contains(producer))
    }) {
        return Err("Query Playbook selectors must belong to the declared producer set".to_owned());
    }
    Ok(ProgressiveQueryRequest::Selector {
        language,
        documents,
        selectors,
        projection,
        output_format: output_format.unwrap_or(QueryOutputFormat::Human),
        workspace,
    })
}

fn lower_producers(form: &[SchemeDatum]) -> Result<(Option<String>, Option<String>), String> {
    if symbol(form.first()) != Some("producers") {
        return Err("expected (producers (language ...) (documents ...))".to_owned());
    }
    let mut language = None;
    let mut documents = None;
    for axis in &form[1..] {
        let SchemeDatum::List(values) = axis else {
            return Err("producer axis must be a list".to_owned());
        };
        let Some(name) = symbol(values.first()) else {
            return Err("producer axis must begin with language or documents".to_owned());
        };
        let atoms = values[1..]
            .iter()
            .map(|value| match value {
                SchemeDatum::Symbol(value) if valid_registered_name(value) => Ok(value.as_str()),
                _ => Err("producer values must be registered-name symbols".to_owned()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if atoms.is_empty() {
            return Err(format!("{name} producer axis is empty"));
        }
        let target = match name {
            "language" => &mut language,
            "documents" => &mut documents,
            other => {
                return Err(format!(
                    "Query Playbook does not support producer axis `{other}`"
                ));
            }
        };
        if target.replace(atoms.join("|")).is_some() {
            return Err(format!("producer axis `{name}` may occur only once"));
        }
    }
    if language.is_none() && documents.is_none() {
        return Err("Query Playbook requires at least one producer axis".to_owned());
    }
    Ok((language, documents))
}

fn symbol(datum: Option<&SchemeDatum>) -> Option<&str> {
    match datum {
        Some(SchemeDatum::Symbol(value)) => Some(value),
        _ => None,
    }
}

fn valid_registered_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-'))
}

fn validate_projection(projection: &str) -> Result<(), String> {
    let valid = matches!(projection, "source" | "callable-skeleton");
    if valid {
        Ok(())
    } else {
        Err("query projection must be source or callable-skeleton".to_owned())
    }
}

#[cfg(test)]
#[path = "../tests/unit/progressive_query.rs"]
mod tests;
