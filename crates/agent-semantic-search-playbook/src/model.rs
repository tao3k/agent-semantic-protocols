// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Stable V1 Search Playbook model, independent from Scheme source parsing.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressiveSearchPlaybookRequest {
    pub language: Option<String>,
    pub documents: Option<String>,
    pub workspace: Option<String>,
    pub rg: Vec<Vec<String>>,
    pub tantivy: Vec<Vec<String>>,
    pub syntax: Vec<ProducerNativeBlock>,
    pub native_syntax: Vec<String>,
    pub graph: Vec<GraphNativeBlock>,
    pub clause_order: Vec<SearchPlaybookClauseRef>,
    pub composition: SearchPlaybookComposition,
    pub normalized_composition: SearchPlaybookNormalizedComposition,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchPlaybookLeaf {
    Rg(Vec<String>),
    Tantivy(Vec<String>),
    Syntax(ProducerNativeBlock),
    NativeSyntax(String),
    Graph(GraphNativeBlock),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchPlaybookComposition {
    Chain(Vec<SearchPlaybookComposition>),
    Intersect(Vec<SearchPlaybookComposition>),
    Leaf(SearchPlaybookLeaf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchPlaybookNormalizedComposition {
    Chain(Vec<SearchPlaybookNormalizedComposition>),
    Intersect(Vec<SearchPlaybookNormalizedComposition>),
    Leaf(SearchPlaybookClauseRef),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchPlaybookCompositionMetrics {
    pub nodes: usize,
    pub depth: usize,
    pub static_work: usize,
}

pub const SEARCH_PLAYBOOK_MAX_COMPOSITION_NODES: usize = 256;
pub const SEARCH_PLAYBOOK_MAX_COMPOSITION_DEPTH: usize = 32;
pub const SEARCH_PLAYBOOK_MAX_STATIC_WORK: usize = 1_024;

impl SearchPlaybookComposition {
    #[must_use]
    pub fn metrics(&self) -> SearchPlaybookCompositionMetrics {
        match self {
            Self::Leaf(_) => SearchPlaybookCompositionMetrics {
                nodes: 1,
                depth: 1,
                static_work: 1,
            },
            Self::Chain(children) | Self::Intersect(children) => {
                let mut nodes: usize = 1;
                let mut depth = 1;
                let mut static_work: usize = if matches!(self, Self::Intersect(_)) {
                    2
                } else {
                    1
                };
                for child in children {
                    let child = child.metrics();
                    nodes = nodes.saturating_add(child.nodes);
                    depth = depth.max(child.depth.saturating_add(1));
                    static_work = static_work.saturating_add(child.static_work);
                }
                SearchPlaybookCompositionMetrics {
                    nodes,
                    depth,
                    static_work,
                }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPlaybookProducerDeclaration {
    pub language: Vec<String>,
    pub documents: Vec<String>,
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
            Self::UnsupportedOption(operator) => write!(
                formatter,
                "Search Playbook does not support operator `{operator}`"
            ),
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
            Self::UnsupportedOption(_) => "search-playbook-operator-unsupported",
            Self::InvalidParameter { reason_kind, .. }
            | Self::InvalidRg { reason_kind, .. }
            | Self::InvalidTantivy { reason_kind, .. } => reason_kind,
        }
    }
}
