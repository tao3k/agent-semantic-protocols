// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Single-layer public Search Playbook grammar.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProgressiveSearchPlaybookRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documents: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rg: Vec<Vec<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tantivy: Vec<Vec<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub syntax: Vec<ProducerNativeBlock>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub native_syntax: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub graph: Vec<GraphNativeBlock>,
    pub clause_order: Vec<SearchPlaybookClauseRef>,
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
    Rg,
    Tantivy,
    Syntax,
    #[serde(rename = "native-syntax")]
    NativeSyntax,
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
    IncompleteRequest(String),
    InvalidClauseOrder(String),
    UnsupportedOption(String),
    InvalidParameter {
        reason_kind: &'static str,
        message: String,
    },
    InvalidRg {
        reason_kind: &'static str,
        message: String,
    },
    InvalidTantivy {
        reason_kind: &'static str,
        message: String,
    },
}

impl std::fmt::Display for ProgressiveSearchPlaybookError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOperation => formatter.write_str("use `asp search playbook`"),
            Self::IncompleteRequest(reason) => {
                write!(formatter, "search-playbook-request-incomplete: {reason}")
            }
            Self::InvalidClauseOrder(reason) => formatter.write_str(reason),
            Self::UnsupportedOption(option) => {
                write!(
                    formatter,
                    "search playbook does not support option `{option}`"
                )
            }
            Self::InvalidParameter { message, .. }
            | Self::InvalidRg { message, .. }
            | Self::InvalidTantivy { message, .. } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ProgressiveSearchPlaybookError {}

impl ProgressiveSearchPlaybookError {
    pub fn reason_kind(&self) -> &'static str {
        match self {
            Self::IncompleteRequest(_) => "search-playbook-request-incomplete",
            Self::InvalidOperation => "search-playbook-operation-invalid",
            Self::InvalidClauseOrder(_) => "search-playbook-clause-order-invalid",
            Self::UnsupportedOption(_) => "search-playbook-option-unsupported",
            Self::InvalidParameter { reason_kind, .. } => reason_kind,
            Self::InvalidRg { reason_kind, .. } => reason_kind,
            Self::InvalidTantivy { reason_kind, .. } => reason_kind,
        }
    }
}

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
    use agent_semantic_shell_parser::{SearchPlaybookBlockKind, SearchPlaybookGlobalKind};

    let parsed = agent_semantic_shell_parser::parse_search_playbook_boundaries(args);
    if let Some(issue) = parsed.issues.first() {
        use agent_semantic_shell_parser::SearchPlaybookIssueKind;
        return Err(match issue.kind {
            SearchPlaybookIssueKind::InvalidOperation => {
                ProgressiveSearchPlaybookError::InvalidOperation
            }
            SearchPlaybookIssueKind::GraphBeforeInput
            | SearchPlaybookIssueKind::InputAfterGraph => {
                ProgressiveSearchPlaybookError::InvalidClauseOrder(issue.message.clone())
            }
            SearchPlaybookIssueKind::MissingValue
            | SearchPlaybookIssueKind::PairedInputRequired
            | SearchPlaybookIssueKind::IncompleteSyntax
            | SearchPlaybookIssueKind::IncompleteGraph
            | SearchPlaybookIssueKind::NativeSyntaxArity => {
                ProgressiveSearchPlaybookError::IncompleteRequest(issue.message.clone())
            }
            SearchPlaybookIssueKind::DuplicateGlobal
            | SearchPlaybookIssueKind::InvalidProducerExpression
            | SearchPlaybookIssueKind::InvalidWorkspaceIdentity
            | SearchPlaybookIssueKind::InvalidSyntaxProducer
            | SearchPlaybookIssueKind::InvalidNativeSelector
            | SearchPlaybookIssueKind::InvalidGraphLanguage => {
                ProgressiveSearchPlaybookError::InvalidParameter {
                    reason_kind: issue.kind.reason_kind(),
                    message: issue.message.clone(),
                }
            }
            SearchPlaybookIssueKind::UnknownOption => {
                ProgressiveSearchPlaybookError::UnsupportedOption(
                    issue
                        .token_index
                        .and_then(|index| args.get(index))
                        .cloned()
                        .unwrap_or_else(|| issue.field.to_owned()),
                )
            }
            SearchPlaybookIssueKind::RgSyntaxInvalid
            | SearchPlaybookIssueKind::RgRootOutsideGeneration
            | SearchPlaybookIssueKind::RgSecondaryProcessForbidden
            | SearchPlaybookIssueKind::RgOperationNotSearch
            | SearchPlaybookIssueKind::RgOutputNotAttributable => {
                ProgressiveSearchPlaybookError::InvalidRg {
                    reason_kind: issue.kind.reason_kind(),
                    message: issue.message.clone(),
                }
            }
            SearchPlaybookIssueKind::TantivySyntaxInvalid
            | SearchPlaybookIssueKind::TantivyFieldUnsupported
            | SearchPlaybookIssueKind::TantivyExpressionTooSimple => {
                ProgressiveSearchPlaybookError::InvalidTantivy {
                    reason_kind: issue.kind.reason_kind(),
                    message: issue.message.clone(),
                }
            }
        });
    }

    let language = parsed
        .global_value(SearchPlaybookGlobalKind::Language)
        .map(str::to_owned);
    let documents = parsed
        .global_value(SearchPlaybookGlobalKind::Documents)
        .map(str::to_owned);
    let workspace = parsed
        .global_value(SearchPlaybookGlobalKind::Workspace)
        .map(str::to_owned);
    let mut rg = Vec::new();
    let mut tantivy = Vec::new();
    let mut syntax = Vec::new();
    let mut native_syntax = Vec::new();
    let mut graph = Vec::new();
    let mut clause_order = Vec::new();

    for block in parsed.blocks {
        match block.kind {
            SearchPlaybookBlockKind::Rg => {
                let block_index = rg.len();
                rg.push(block.argv);
                clause_order.push(SearchPlaybookClauseRef {
                    axis: SearchPlaybookClauseAxis::Rg,
                    block_index,
                });
            }
            SearchPlaybookBlockKind::Tantivy => {
                let block_index = tantivy.len();
                tantivy.push(block.argv);
                clause_order.push(SearchPlaybookClauseRef {
                    axis: SearchPlaybookClauseAxis::Tantivy,
                    block_index,
                });
            }
            SearchPlaybookBlockKind::Syntax => {
                let mut argv = block.argv;
                let producer = argv.remove(0);
                let block_index = syntax.len();
                syntax.push(ProducerNativeBlock { producer, argv });
                clause_order.push(SearchPlaybookClauseRef {
                    axis: SearchPlaybookClauseAxis::Syntax,
                    block_index,
                });
            }
            SearchPlaybookBlockKind::NativeSyntax => {
                let block_index = native_syntax.len();
                native_syntax.push(block.argv.into_iter().next().expect("validated selector"));
                clause_order.push(SearchPlaybookClauseRef {
                    axis: SearchPlaybookClauseAxis::NativeSyntax,
                    block_index,
                });
            }
            SearchPlaybookBlockKind::Graph => {
                let mut argv = block.argv;
                let language = argv.remove(0);
                let block_index = graph.len();
                graph.push(GraphNativeBlock { language, argv });
                clause_order.push(SearchPlaybookClauseRef {
                    axis: SearchPlaybookClauseAxis::Graph,
                    block_index,
                });
            }
        }
    }

    Ok(ProgressiveSearchPlaybookRequest {
        language,
        documents,
        workspace,
        rg,
        tantivy,
        syntax,
        native_syntax,
        graph,
        clause_order,
    })
}

#[cfg(test)]
#[path = "../tests/unit/progressive_playbook.rs"]
mod tests;
