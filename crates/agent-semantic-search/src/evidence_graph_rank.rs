//! EvidenceGraph node ranking policy for graph-route seeds.

use std::cmp::Reverse;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceGraphRankNode {
    pub ordinal: usize,
    pub id: String,
    pub kind: String,
    pub label: String,
    pub path: Option<String>,
    pub selector: Option<String>,
    pub query_keys: Vec<String>,
    pub outgoing_edge_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceGraphRankedNode {
    pub node: EvidenceGraphRankNode,
    pub score: EvidenceGraphRankScore,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct EvidenceGraphRankScore {
    pub term_hits: usize,
    pub selector_bonus: usize,
    pub topology_bonus: usize,
}

#[must_use]
pub fn rank_evidence_graph_nodes(
    nodes: Vec<EvidenceGraphRankNode>,
    intent: &str,
) -> Vec<EvidenceGraphRankedNode> {
    let terms = evidence_graph_rank_terms(intent);
    let mut ranked = nodes
        .into_iter()
        .enumerate()
        .map(|(index, node)| {
            let score = evidence_graph_rank_score(&node, terms.as_slice());
            (index, EvidenceGraphRankedNode { node, score })
        })
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(index, ranked)| evidence_graph_sort_key(ranked, *index));
    ranked.into_iter().map(|(_index, ranked)| ranked).collect()
}

#[must_use]
pub fn evidence_graph_rank_terms(intent: &str) -> Vec<String> {
    let mut terms = BTreeSet::new();
    for term in intent
        .split(|character: char| {
            !(character == '_'
                || character == '-'
                || character == ':'
                || character == '/'
                || character.is_ascii_alphanumeric())
        })
        .map(str::trim)
        .filter(|term| !term.is_empty())
    {
        terms.insert(term.to_ascii_lowercase());
    }
    terms.into_iter().collect()
}

fn evidence_graph_rank_score(
    node: &EvidenceGraphRankNode,
    terms: &[String],
) -> EvidenceGraphRankScore {
    EvidenceGraphRankScore {
        term_hits: evidence_graph_term_hits(node, terms),
        selector_bonus: usize::from(
            node.selector
                .as_ref()
                .is_some_and(|selector| !selector.trim().is_empty()),
        ),
        topology_bonus: node.outgoing_edge_count.min(8),
    }
}

fn evidence_graph_term_hits(node: &EvidenceGraphRankNode, terms: &[String]) -> usize {
    let id = node.id.to_ascii_lowercase();
    let kind = node.kind.to_ascii_lowercase();
    let label = node.label.to_ascii_lowercase();
    let path = node
        .path
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let selector = node
        .selector
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    terms
        .iter()
        .filter(|term| {
            id.contains(term.as_str())
                || kind.contains(term.as_str())
                || label.contains(term.as_str())
                || path.contains(term.as_str())
                || selector.contains(term.as_str())
                || node
                    .query_keys
                    .iter()
                    .any(|key| key.to_ascii_lowercase().contains(term.as_str()))
        })
        .count()
}

type EvidenceGraphSortKey = (Reverse<usize>, Reverse<usize>, Reverse<usize>, usize);

fn evidence_graph_sort_key(ranked: &EvidenceGraphRankedNode, index: usize) -> EvidenceGraphSortKey {
    (
        Reverse(ranked.score.term_hits),
        Reverse(ranked.score.selector_bonus),
        Reverse(ranked.score.topology_bonus),
        index,
    )
}
