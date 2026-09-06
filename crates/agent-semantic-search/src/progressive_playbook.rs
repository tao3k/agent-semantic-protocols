//! Single-layer public Search Playbook grammar.

use serde::{Deserialize, Serialize};

const PLAYBOOK_OPTIONS: &[&str] = &[
    "--languages",
    "--documents",
    "--workspace",
    "--fd",
    "--rg",
    "--tantivy",
    "--syntax",
    "--graph",
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ProgressiveSearchPlaybookRequest {
    ContractQuery {
        #[serde(skip_serializing_if = "Option::is_none")]
        languages: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        documents: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace: Option<String>,
    },
    Execute {
        #[serde(skip_serializing_if = "Option::is_none")]
        languages: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        documents: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace: Option<String>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        fd: Vec<Vec<String>>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        rg: Vec<Vec<String>>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        tantivy: Vec<Vec<String>>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        syntax: Vec<ProducerNativeBlock>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        graph: Vec<GraphNativeBlock>,
        clause_order: Vec<SearchPlaybookClauseRef>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProducerNativeBlock {
    pub producer: String,
    pub argv: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphNativeBlock {
    pub language: String,
    pub argv: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchPlaybookClauseAxis {
    Fd,
    Rg,
    Tantivy,
    Syntax,
    Graph,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookClauseRef {
    pub axis: SearchPlaybookClauseAxis,
    pub block_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgressiveSearchPlaybookError {
    InvalidOperation,
    InvalidClauseOrder(String),
    UnsupportedOption(String),
}

impl std::fmt::Display for ProgressiveSearchPlaybookError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOperation => formatter.write_str("use `asp search playbook`"),
            Self::InvalidClauseOrder(reason) => formatter.write_str(reason),
            Self::UnsupportedOption(option) => {
                write!(
                    formatter,
                    "search playbook does not support option `{option}`"
                )
            }
        }
    }
}

impl std::error::Error for ProgressiveSearchPlaybookError {}

pub fn parse_progressive_search_playbook_args(
    args: &[String],
) -> Result<ProgressiveSearchPlaybookRequest, ProgressiveSearchPlaybookError> {
    if args.first().map(String::as_str) != Some("search")
        || args.get(1).map(String::as_str) != Some("playbook")
    {
        return Err(ProgressiveSearchPlaybookError::InvalidOperation);
    }
    parse_playbook(args)
}

fn parse_playbook(
    args: &[String],
) -> Result<ProgressiveSearchPlaybookRequest, ProgressiveSearchPlaybookError> {
    let mut languages = None;
    let mut documents = None;
    let mut workspace = None;
    let mut fd = Vec::new();
    let mut rg = Vec::new();
    let mut tantivy = Vec::new();
    let mut syntax = Vec::new();
    let mut graph = Vec::new();
    let mut clause_order = Vec::new();
    let mut graph_started = false;
    let mut index = 2;

    while index < args.len() {
        match args[index].as_str() {
            "--languages" => languages = Some(single_value(args, &mut index, "--languages")?),
            "--documents" => documents = Some(single_value(args, &mut index, "--documents")?),
            "--workspace" => workspace = Some(single_value(args, &mut index, "--workspace")?),
            "--fd" => {
                reject_acquisition_after_graph(graph_started)?;
                let block_index = fd.len();
                fd.push(native_block(args, &mut index, "--fd")?);
                clause_order.push(SearchPlaybookClauseRef {
                    axis: SearchPlaybookClauseAxis::Fd,
                    block_index,
                });
            }
            "--rg" => {
                reject_acquisition_after_graph(graph_started)?;
                let block_index = rg.len();
                rg.push(native_block(args, &mut index, "--rg")?);
                clause_order.push(SearchPlaybookClauseRef {
                    axis: SearchPlaybookClauseAxis::Rg,
                    block_index,
                });
            }
            "--tantivy" => {
                reject_acquisition_after_graph(graph_started)?;
                let block_index = tantivy.len();
                tantivy.push(native_block(args, &mut index, "--tantivy")?);
                clause_order.push(SearchPlaybookClauseRef {
                    axis: SearchPlaybookClauseAxis::Tantivy,
                    block_index,
                });
            }
            "--syntax" => {
                reject_acquisition_after_graph(graph_started)?;
                let mut block = native_block(args, &mut index, "--syntax")?;
                if block.len() < 2 {
                    return Ok(ProgressiveSearchPlaybookRequest::ContractQuery {
                        languages,
                        documents,
                        workspace,
                    });
                }
                let producer = block.remove(0);
                let block_index = syntax.len();
                syntax.push(ProducerNativeBlock {
                    producer,
                    argv: block,
                });
                clause_order.push(SearchPlaybookClauseRef {
                    axis: SearchPlaybookClauseAxis::Syntax,
                    block_index,
                });
            }
            "--graph" => {
                if clause_order.is_empty() {
                    return Ok(ProgressiveSearchPlaybookRequest::ContractQuery {
                        languages,
                        documents,
                        workspace,
                    });
                }
                let mut block = native_block(args, &mut index, "--graph")?;
                if block.len() < 2 {
                    return Ok(ProgressiveSearchPlaybookRequest::ContractQuery {
                        languages,
                        documents,
                        workspace,
                    });
                }
                let language = block.remove(0);
                let block_index = graph.len();
                graph.push(GraphNativeBlock {
                    language,
                    argv: block,
                });
                clause_order.push(SearchPlaybookClauseRef {
                    axis: SearchPlaybookClauseAxis::Graph,
                    block_index,
                });
                graph_started = true;
            }
            option => {
                return Err(ProgressiveSearchPlaybookError::UnsupportedOption(
                    option.to_owned(),
                ));
            }
        }
    }

    validate_producer_expression(languages.as_deref())?;
    validate_producer_expression(documents.as_deref())?;

    let has_acquisition = clause_order
        .iter()
        .any(|clause| clause.axis != SearchPlaybookClauseAxis::Graph);
    if (languages.is_none() && documents.is_none()) || !has_acquisition {
        return Ok(ProgressiveSearchPlaybookRequest::ContractQuery {
            languages,
            documents,
            workspace,
        });
    }

    Ok(ProgressiveSearchPlaybookRequest::Execute {
        languages,
        documents,
        workspace,
        fd,
        rg,
        tantivy,
        syntax,
        graph,
        clause_order,
    })
}

fn reject_acquisition_after_graph(
    graph_started: bool,
) -> Result<(), ProgressiveSearchPlaybookError> {
    if graph_started {
        return Err(ProgressiveSearchPlaybookError::InvalidClauseOrder(
            "Search Playbook acquisition clauses must precede --graph".to_owned(),
        ));
    }
    Ok(())
}

fn single_value(
    args: &[String],
    index: &mut usize,
    option: &str,
) -> Result<String, ProgressiveSearchPlaybookError> {
    let value = args.get(*index + 1).filter(|value| !is_boundary(value));
    let Some(value) = value else {
        return Err(ProgressiveSearchPlaybookError::UnsupportedOption(
            option.to_owned(),
        ));
    };
    *index += 2;
    Ok(value.clone())
}

fn native_block(
    args: &[String],
    index: &mut usize,
    option: &str,
) -> Result<Vec<String>, ProgressiveSearchPlaybookError> {
    let start = *index + 1;
    let mut end = start;
    while end < args.len() && !is_boundary(&args[end]) {
        end += 1;
    }
    if start == end {
        return Err(ProgressiveSearchPlaybookError::UnsupportedOption(
            option.to_owned(),
        ));
    }
    *index = end;
    Ok(args[start..end].to_vec())
}

fn is_boundary(value: &str) -> bool {
    PLAYBOOK_OPTIONS.contains(&value)
}

fn validate_producer_expression(
    expression: Option<&str>,
) -> Result<(), ProgressiveSearchPlaybookError> {
    let Some(expression) = expression else {
        return Ok(());
    };
    if expression
        .split('|')
        .any(|producer| producer.trim().is_empty() || producer.trim() != producer)
    {
        return Err(ProgressiveSearchPlaybookError::UnsupportedOption(
            "invalid producer expression".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/progressive_playbook.rs"]
mod tests;
