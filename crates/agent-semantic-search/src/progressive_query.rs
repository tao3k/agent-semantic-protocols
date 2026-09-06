//! Direct syntax-query and exact-selector grammar for the shared Query surface.

use serde::{Deserialize, Serialize};

use crate::ProducerNativeBlock;

const QUERY_OPTIONS: &[&str] = &[
    "--selector",
    "--languages",
    "--documents",
    "--workspace",
    "--syntax",
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
        selector: String,
        projection: String,
        output_format: QueryOutputFormat,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace: Option<String>,
    },
    Syntax {
        #[serde(skip_serializing_if = "Option::is_none")]
        languages: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        documents: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace: Option<String>,
        syntax: Vec<ProducerNativeBlock>,
        projection: String,
        output_format: QueryOutputFormat,
    },
}

pub fn parse_progressive_query_args(args: &[String]) -> Result<ProgressiveQueryRequest, String> {
    if args.first().map(String::as_str) != Some("query") {
        return Err("use `asp query`".to_owned());
    }

    let mut selector = None;
    let mut languages = None;
    let mut documents = None;
    let mut workspace = None;
    let mut syntax = Vec::new();
    let mut projection = None;
    let output_format = query_output_format(args);
    // `playbook` is the explicit agent-facing Query facade marker.  Keep the
    // legacy marker-less spelling as a compatibility seam, but never treat the
    // marker as a provider option or forward it into Runtime admission.
    let mut index = if args.get(1).map(String::as_str) == Some("playbook") {
        2
    } else {
        1
    };

    while index < args.len() {
        match args[index].as_str() {
            "--selector" => selector = Some(single_value(args, &mut index, "--selector")?),
            "--languages" => {
                languages = Some(single_value(args, &mut index, "--languages")?);
            }
            "--documents" => {
                documents = Some(single_value(args, &mut index, "--documents")?);
            }
            "--workspace" => {
                workspace = Some(single_value(args, &mut index, "--workspace")?);
            }
            "--projection" => {
                projection = Some(single_value(args, &mut index, "--projection")?);
            }
            "--json" => index += 1,
            "--syntax" => {
                let mut block = native_block(args, &mut index)?;
                if block.len() < 2 {
                    return Err(
                        "Example: --syntax rust --treesitter-query '((identifier) @symbol)'\nGrammar: --syntax <registered-producer> <native syntax query tokens...>"
                            .to_owned(),
                    );
                }
                let producer = block.remove(0);
                syntax.push(ProducerNativeBlock {
                    producer,
                    argv: block,
                });
            }
            option => return Err(format!("query does not support option `{option}`")),
        }
    }

    match (selector, syntax.is_empty()) {
        (Some(selector), true) if languages.is_none() && documents.is_none() => {
            let projection = projection.unwrap_or_else(|| "source".to_owned());
            validate_projection(&projection, false)?;
            Ok(ProgressiveQueryRequest::Selector {
                selector,
                projection,
                output_format,
                workspace,
            })
        }
        (None, false) if languages.is_some() || documents.is_some() => {
            validate_producer_expression(languages.as_deref())?;
            validate_producer_expression(documents.as_deref())?;
            let projection = projection.unwrap_or_else(|| "matches".to_owned());
            validate_projection(&projection, true)?;
            Ok(ProgressiveQueryRequest::Syntax {
                languages,
                documents,
                workspace,
                syntax,
                projection,
                output_format,
            })
        }
        _ => Err(
            "query requires exactly one mode: --selector, or producer selectors plus --syntax"
                .to_owned(),
        ),
    }
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

fn native_block(args: &[String], index: &mut usize) -> Result<Vec<String>, String> {
    let start = *index + 1;
    let mut end = start;
    while end < args.len() && !QUERY_OPTIONS.contains(&args[end].as_str()) {
        end += 1;
    }
    if start == end {
        return Err("--syntax requires a native block".to_owned());
    }
    *index = end;
    Ok(args[start..end].to_vec())
}

fn validate_producer_expression(expression: Option<&str>) -> Result<(), String> {
    let Some(expression) = expression else {
        return Ok(());
    };
    if expression
        .split('|')
        .any(|producer| producer.trim().is_empty() || producer.trim() != producer)
    {
        return Err("producer expression must be '<producer>(|<producer>)*'".to_owned());
    }
    Ok(())
}

fn validate_projection(projection: &str, syntax: bool) -> Result<(), String> {
    let valid =
        matches!(projection, "source" | "callable-skeleton") || syntax && projection == "matches";
    if valid {
        Ok(())
    } else {
        Err("query projection must be matches, source, or callable-skeleton".to_owned())
    }
}

#[cfg(test)]
#[path = "../tests/unit/progressive_query.rs"]
mod tests;
