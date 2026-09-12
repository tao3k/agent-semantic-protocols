// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Direct syntax-query and exact-selector grammar for the shared Query surface.

use serde::{Deserialize, Serialize};

const QUERY_OPTIONS: &[&str] = &[
    "--language",
    "--documents",
    "--selector",
    "--workspace",
    "--projection",
    "--json",
];

/// Presentation selected for the public Query surface.
///
/// Query is an agent-facing source/syntax operation by default.  The typed
/// Runtime packet is an implementation detail unless the caller explicitly
/// requests it with `--json`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueryOutputFormat {
    Human,
    Json,
}

/// Resolve the public presentation contract for either root or language-facade
/// Query arguments.  Only an explicit `--json` opts into wire output.
pub fn query_output_format(args: &[String]) -> QueryOutputFormat {
    if args.iter().any(|argument| argument == "--json") {
        QueryOutputFormat::Json
    } else {
        QueryOutputFormat::Human
    }
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
    if args.first().map(String::as_str) != Some("query") {
        return Err("use `asp query playbook`".to_owned());
    }

    if args.get(1).map(String::as_str) != Some("playbook") {
        return Err("use `asp query playbook`".to_owned());
    }

    let mut selectors = Vec::new();
    let mut language = None;
    let mut documents = None;
    let mut workspace = None;
    let mut projection = None;
    let output_format = query_output_format(args);
    let mut index = 2;

    while index < args.len() {
        match args[index].as_str() {
            "--language" => {
                if language.is_some() {
                    return Err("query playbook accepts exactly one --language".to_owned());
                }
                language = Some(single_value(args, &mut index, "--language")?);
            }
            "--documents" => {
                if documents.is_some() {
                    return Err("query playbook accepts exactly one --documents".to_owned());
                }
                documents = Some(single_value(args, &mut index, "--documents")?);
            }
            "--selector" => selectors.push(single_value(args, &mut index, "--selector")?),
            "--workspace" => {
                workspace = Some(single_value(args, &mut index, "--workspace")?);
            }
            "--projection" => {
                projection = Some(single_value(args, &mut index, "--projection")?);
            }
            "--json" => index += 1,
            option => return Err(format!("query playbook does not support option `{option}`")),
        }
    }

    if language.is_none() && documents.is_none() {
        return Err("query playbook requires --language or --documents".to_owned());
    }
    if language
        .as_deref()
        .is_some_and(|value| !valid_producer_expression(value))
    {
        return Err(
            "query playbook --language must be a registered-producer expression".to_owned(),
        );
    }
    if documents
        .as_deref()
        .is_some_and(|value| !valid_producer_expression(value))
    {
        return Err(
            "query playbook --documents must be a registered-producer expression".to_owned(),
        );
    }
    if selectors.is_empty() {
        return Err("query playbook requires at least one --selector".to_owned());
    }
    if selectors
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != selectors.len()
    {
        return Err("query playbook selectors must be unique".to_owned());
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
        return Err(
            "query playbook selectors must belong to the declared --language/--documents producer set".to_owned(),
        );
    }
    Ok(ProgressiveQueryRequest::Selector {
        language,
        documents,
        selectors,
        projection,
        output_format,
        workspace,
    })
}

fn valid_producer_expression(value: &str) -> bool {
    !value.is_empty()
        && value.split('|').all(|producer| {
            producer
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
                && producer.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-')
                })
        })
}

fn single_value(args: &[String], index: &mut usize, option: &str) -> Result<String, String> {
    let value = args
        .get(*index + 1)
        .filter(|value| !QUERY_OPTIONS.contains(&value.as_str()))
        .cloned()
        .ok_or_else(|| format!("{option} requires a value"))?;
    *index += 2;
    Ok(value)
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
