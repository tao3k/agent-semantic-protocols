//! Dirty-source overlay search for session-scoped dynamic evidence.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

pub const QUERY_OVERLAY_ROUTE_SOURCE: &str = "query-overlay";
pub const SEARCH_OVERLAY_ROUTE_SOURCE: &str = "search-overlay";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DynamicOverlayLane {
    Query,
    Search,
}

impl DynamicOverlayLane {
    #[must_use]
    pub fn route_source(self) -> &'static str {
        match self {
            Self::Query => QUERY_OVERLAY_ROUTE_SOURCE,
            Self::Search => SEARCH_OVERLAY_ROUTE_SOURCE,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct DynamicOverlayNamespace {
    pub(crate) project_id: String,
    pub(crate) workspace_id: String,
    pub(crate) worktree_id: String,
    pub(crate) session_id: String,
    pub(crate) subagent_id: Option<String>,
    pub(crate) base_generation: String,
}

impl DynamicOverlayNamespace {
    #[must_use]
    pub(crate) fn new(
        project_id: impl Into<String>,
        workspace_id: impl Into<String>,
        worktree_id: impl Into<String>,
        session_id: impl Into<String>,
        base_generation: impl Into<String>,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            workspace_id: workspace_id.into(),
            worktree_id: worktree_id.into(),
            session_id: session_id.into(),
            subagent_id: None,
            base_generation: base_generation.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DynamicOverlayDocument {
    pub(crate) owner_path: String,
    pub(crate) entity_id: String,
    pub(crate) selector: String,
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) signature: Option<String>,
    pub(crate) display_range: Option<(usize, usize)>,
    pub(crate) source_hash: String,
    pub(crate) search_text: String,
}

impl DynamicOverlayDocument {
    #[must_use]
    pub(crate) fn owner_item(
        owner_path: impl Into<String>,
        kind: impl Into<String>,
        name: impl Into<String>,
        start: usize,
        end: usize,
        source_hash: impl Into<String>,
    ) -> Self {
        let owner_path = owner_path.into();
        let kind = kind.into();
        let name = name.into();
        let selector = format!(
            "dynamic-overlay://{owner_path}#item/{}/{}",
            kind,
            name.replace(char::is_whitespace, "-")
        );
        Self {
            owner_path: owner_path.clone(),
            entity_id: selector.clone(),
            selector,
            kind: kind.clone(),
            name: name.clone(),
            signature: None,
            display_range: Some((start, end.max(start))),
            source_hash: source_hash.into(),
            search_text: expanded_identifier_text(&[&owner_path, &kind, &name]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DynamicOverlayQuery {
    pub(crate) text: String,
    pub(crate) owner_path: Option<String>,
    pub(crate) limit: usize,
}

impl DynamicOverlayQuery {
    #[must_use]
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            owner_path: None,
            limit: 8,
        }
    }

    #[must_use]
    pub(crate) fn owner_path(mut self, owner_path: impl Into<String>) -> Self {
        self.owner_path = Some(owner_path.into());
        self
    }

    #[must_use]
    pub(crate) fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{DynamicOverlayLane, QUERY_OVERLAY_ROUTE_SOURCE, SEARCH_OVERLAY_ROUTE_SOURCE};

    #[test]
    fn dynamic_overlay_lanes_expose_search_and_query_route_sources() {
        assert_eq!(
            DynamicOverlayLane::Search.route_source(),
            SEARCH_OVERLAY_ROUTE_SOURCE
        );
        assert_eq!(
            DynamicOverlayLane::Query.route_source(),
            QUERY_OVERLAY_ROUTE_SOURCE
        );
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DynamicOverlaySearchHit {
    pub(crate) document: DynamicOverlayDocument,
    pub(crate) score: f32,
    pub(crate) matched_terms: Vec<String>,
}

pub(crate) trait DynamicOverlaySearchBackend {
    fn upsert_documents(
        &mut self,
        namespace: DynamicOverlayNamespace,
        documents: Vec<DynamicOverlayDocument>,
    );

    fn search(
        &self,
        namespace: &DynamicOverlayNamespace,
        query: &DynamicOverlayQuery,
    ) -> Vec<DynamicOverlaySearchHit>;
}

pub(crate) fn default_dynamic_overlay_search_backend() -> Box<dyn DynamicOverlaySearchBackend> {
    Box::new(InMemoryDynamicOverlaySearch::default())
}

#[derive(Default)]
pub(crate) struct InMemoryDynamicOverlaySearch {
    documents: BTreeMap<DynamicOverlayNamespace, BTreeMap<String, DynamicOverlayDocument>>,
}

impl DynamicOverlaySearchBackend for InMemoryDynamicOverlaySearch {
    fn upsert_documents(
        &mut self,
        namespace: DynamicOverlayNamespace,
        documents: Vec<DynamicOverlayDocument>,
    ) {
        let namespace_documents = self.documents.entry(namespace).or_default();
        for document in documents {
            namespace_documents.insert(document.entity_id.clone(), document);
        }
    }

    fn search(
        &self,
        namespace: &DynamicOverlayNamespace,
        query: &DynamicOverlayQuery,
    ) -> Vec<DynamicOverlaySearchHit> {
        let terms = overlay_query_terms(&query.text);
        let mut hits = self
            .documents
            .get(namespace)
            .into_iter()
            .flat_map(BTreeMap::values)
            .filter(|document| {
                query
                    .owner_path
                    .as_ref()
                    .is_none_or(|owner_path| owner_path == &document.owner_path)
            })
            .filter_map(|document| score_document(document, &terms))
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.document.owner_path.cmp(&right.document.owner_path))
                .then_with(|| left.document.name.cmp(&right.document.name))
        });
        hits.truncate(query.limit.max(1));
        hits
    }
}

fn score_document(
    document: &DynamicOverlayDocument,
    terms: &[String],
) -> Option<DynamicOverlaySearchHit> {
    let haystack = expanded_identifier_text(&[
        &document.owner_path,
        &document.kind,
        &document.name,
        document.signature.as_deref().unwrap_or_default(),
        &document.search_text,
    ]);
    let haystack_terms = token_set(&haystack);
    let matched_terms = terms
        .iter()
        .filter(|term| haystack_terms.contains(term.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !terms.is_empty() && matched_terms.len() != terms.len() {
        return None;
    }
    let lower_name = document.name.to_ascii_lowercase();
    let exact_name = terms.iter().any(|term| term == &lower_name);
    let prefix_name = terms.iter().any(|term| lower_name.starts_with(term));
    let score = (matched_terms.len() as f32 * 10.0)
        + if exact_name { 20.0 } else { 0.0 }
        + if prefix_name { 5.0 } else { 0.0 };
    Some(DynamicOverlaySearchHit {
        document: document.clone(),
        score,
        matched_terms,
    })
}

fn token_set(text: &str) -> BTreeSet<&str> {
    text.split_ascii_whitespace().collect()
}

fn overlay_query_terms(query: &str) -> Vec<String> {
    expanded_identifier_text(&[query])
        .split_ascii_whitespace()
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn expanded_identifier_text(values: &[&str]) -> String {
    values
        .iter()
        .flat_map(|value| identifier_tokens(value))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(" ")
}

fn identifier_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for raw in value.split(|ch: char| !ch.is_ascii_alphanumeric()) {
        if raw.is_empty() {
            continue;
        }
        tokens.push(raw.to_ascii_lowercase());
        tokens.extend(split_identifier(raw));
    }
    tokens
}

fn split_identifier(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut previous_lowercase = false;
    for ch in value.chars() {
        if ch == '_' || ch == '-' {
            if !current.is_empty() {
                tokens.push(current.to_ascii_lowercase());
                current.clear();
            }
            previous_lowercase = false;
            continue;
        }
        if previous_lowercase && ch.is_ascii_uppercase() && !current.is_empty() {
            tokens.push(current.to_ascii_lowercase());
            current.clear();
        }
        previous_lowercase = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        current.push(ch);
    }
    if !current.is_empty() {
        tokens.push(current.to_ascii_lowercase());
    }
    tokens
}
