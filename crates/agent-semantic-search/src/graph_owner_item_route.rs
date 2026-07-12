//! Parser-owned owner-item ranking over a materialized EvidenceGraph slice.

use std::collections::BTreeSet;

use agent_semantic_client_db::TursoClientDbGraphEntity;

/// Parser-owned semantic kind carried by a selector node.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GraphSemanticKind(String);

impl GraphSemanticKind {
    /// Construct a nonempty parser-owned semantic kind.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.trim().is_empty()).then_some(Self(value))
    }

    /// Return the stable parser-owned kind label.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Graph-owned item evidence that is safe to hand to an exact code query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphOwnerItemEvidence {
    pub node_id: String,
    pub owner_path: String,
    pub symbol: String,
    pub semantic_kind: GraphSemanticKind,
    pub selector: String,
}

/// Result of ranking a bounded owner-local EvidenceGraph slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphOwnerItemRoute {
    Hit(Vec<GraphOwnerItemEvidence>),
    Empty,
}

/// Compact renderer input for a graph-owned owner-item result.
#[derive(Clone, Copy, Debug)]
pub struct GraphOwnerItemRenderRequest<'a> {
    pub language_id: &'a str,
    pub owner_path: &'a str,
    pub query: &'a str,
    pub route: &'a GraphOwnerItemRoute,
}

/// Request for owner-local ranking over parser-owned graph selector nodes.
#[derive(Clone, Copy, Debug)]
pub struct GraphOwnerItemRouteRequest<'a> {
    pub owner_path: &'a str,
    pub query_terms: &'a [String],
    pub nodes: &'a [TursoClientDbGraphEntity],
}

/// Rank exact selector nodes without inspecting source text or selector syntax.
#[must_use]
pub fn rank_graph_owner_items(request: GraphOwnerItemRouteRequest<'_>) -> GraphOwnerItemRoute {
    let mut ranked = request
        .nodes
        .iter()
        .filter_map(|node| {
            let evidence = graph_owner_item_evidence(node, request.owner_path)?;
            let score = graph_owner_item_score(node, &evidence, request.query_terms)?;
            Some((score, evidence))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.symbol.cmp(&right.symbol))
            .then_with(|| left.selector.cmp(&right.selector))
    });
    let mut seen_selectors = BTreeSet::new();
    let evidence = ranked
        .into_iter()
        .map(|(_, evidence)| evidence)
        .filter(|evidence| seen_selectors.insert(evidence.selector.clone()))
        .collect::<Vec<_>>();
    if evidence.is_empty() {
        GraphOwnerItemRoute::Empty
    } else {
        GraphOwnerItemRoute::Hit(evidence)
    }
}

/// Render compact exact-selector evidence without source excerpts or line ranges.
#[must_use]
pub fn render_graph_owner_item_frontier(request: GraphOwnerItemRenderRequest<'_>) -> String {
    let mut output = format!(
        "[search-owner] q={} owner={} selector=items alg=graph-turbo-owner-items\n",
        request.query, request.owner_path
    );
    let GraphOwnerItemRoute::Hit(items) = request.route else {
        output.push_str("reason=no-owner-item-match\n");
        output.push_str("entries=owner-query(O,Q=>turso-evidence-graph)\n");
        return output;
    };
    for (index, item) in items.iter().enumerate() {
        let item_id = if index == 0 {
            "I".to_string()
        } else {
            format!("I{}", index + 1)
        };
        output.push_str(&format!(
            "{item_id}=item:symbol({})@{}!syntax\n",
            item.symbol, item.selector
        ));
        output.push_str(&format!(
            "|item symbol={} kind={} structuralSelector={} graphNode={} reason=graph-owner-item-ready\n",
            item.symbol,
            item.semantic_kind.as_str(),
            item.selector,
            item.node_id
        ));
    }
    if let Some(item) = items.first() {
        output.push_str(&format!(
            "nextCommand=asp {} query --from-hook item-skeleton --selector {} --workspace . --code\n",
            request.language_id,
            shell_quote(&item.selector)
        ));
    }
    output.push_str("reason=graph-owner-item-ready\n");
    output.push_str("avoid=selector-code-before-exact,direct-source-read,manual-window-scan\n");
    output.push_str("entries=owner-query(O,Q=>turso-evidence-graph)\n");
    output
}

fn graph_owner_item_evidence(
    node: &TursoClientDbGraphEntity,
    owner_path: &str,
) -> Option<GraphOwnerItemEvidence> {
    if node.kind != "selector" || node.path.as_deref() != Some(owner_path) {
        return None;
    }
    Some(GraphOwnerItemEvidence {
        node_id: node.id.clone(),
        owner_path: owner_path.to_string(),
        symbol: node.label.clone(),
        semantic_kind: GraphSemanticKind::new(node.semantic_kind.clone()?)?,
        selector: node.selector.clone()?,
    })
}

fn graph_owner_item_score(
    node: &TursoClientDbGraphEntity,
    evidence: &GraphOwnerItemEvidence,
    query_terms: &[String],
) -> Option<u32> {
    let identity_terms = std::iter::once(normalized_graph_term(&evidence.symbol))
        .chain(std::iter::once(normalized_graph_term(
            evidence.semantic_kind.as_str(),
        )))
        .chain(node.query_keys.iter().map(|key| normalized_graph_term(key)))
        .filter(|term| !term.is_empty())
        .collect::<BTreeSet<_>>();
    let query_terms = query_terms
        .iter()
        .map(|term| normalized_graph_term(term))
        .filter(|term| !term.is_empty())
        .collect::<BTreeSet<_>>();
    if !identity_terms.is_superset(&query_terms) {
        return None;
    }
    Some((query_terms.len() as u32) * 4)
}

fn normalized_graph_term(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
