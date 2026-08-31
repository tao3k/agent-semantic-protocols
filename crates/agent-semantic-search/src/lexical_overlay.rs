//! Reusable lexical overlay search for high-churn candidate evidence.

use crate::dynamic_overlay::{
    DynamicOverlayDocument, DynamicOverlayNamespace, DynamicOverlayQuery,
    default_dynamic_overlay_search_backend,
};

/// Request for session-local lexical overlay search.
#[derive(Debug, Clone)]
pub struct LexicalOverlaySearchRequest {
    query: String,
    limit: usize,
    documents: Vec<LexicalOverlayDocument>,
    source_snapshot: agent_semantic_artifacts::SourceSnapshotEvidence,
}

/// Lexical overlay hits bound to the Merkle snapshot that was searched.
#[derive(Debug, Clone)]
pub struct LexicalOverlaySearchResult {
    /// Snapshot evidence for the base plus editor-buffer delta.
    pub source_snapshot: agent_semantic_artifacts::SourceSnapshotEvidence,
    /// Ranked lexical hits from that snapshot.
    pub hits: Vec<LexicalOverlaySearchHit>,
}

/// File-level lexical candidates bound to the Merkle snapshot that produced them.
#[derive(Debug, Clone)]
pub struct LexicalOverlayCandidateSearchResult {
    /// Snapshot evidence for the base plus editor-buffer delta.
    pub source_snapshot: agent_semantic_artifacts::SourceSnapshotEvidence,
    /// File-level candidates projected from the lexical hits.
    pub candidates: Vec<LexicalOverlayCandidateHit>,
}

impl LexicalOverlaySearchRequest {
    /// Create a lexical overlay request.
    #[must_use]
    pub fn new(
        query: impl Into<String>,
        source_snapshot: agent_semantic_artifacts::SourceSnapshotEvidence,
    ) -> Self {
        Self {
            query: query.into(),
            limit: 64,
            documents: Vec::new(),
            source_snapshot,
        }
    }

    /// Set the maximum number of hits returned by this request.
    #[must_use]
    pub fn limit(mut self, value: usize) -> Self {
        self.limit = value;
        self
    }

    /// Add one overlay document.
    #[must_use]
    pub fn document(mut self, document: LexicalOverlayDocument) -> Self {
        self.documents.push(document);
        self
    }
}

/// Searchable document for lexical overlay search.
#[derive(Debug, Clone)]
pub struct LexicalOverlayDocument {
    owner_path: String,
    selector: String,
    kind: String,
    name: String,
    source_hash: String,
    search_text: String,
}

impl LexicalOverlayDocument {
    /// Create a lexical overlay document with stable owner and selector identity.
    #[must_use]
    pub fn new(
        owner_path: impl Into<String>,
        selector: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        let name = name.into();
        Self {
            owner_path: owner_path.into(),
            selector: selector.into(),
            kind: "candidate".to_string(),
            source_hash: "overlay".to_string(),
            search_text: name.clone(),
            name,
        }
    }

    /// Set provider-neutral document kind.
    #[must_use]
    pub fn kind(mut self, value: impl Into<String>) -> Self {
        self.kind = value.into();
        self
    }

    /// Set a source hash or generation fingerprint.
    #[must_use]
    pub fn source_hash(mut self, value: impl Into<String>) -> Self {
        self.source_hash = value.into();
        self
    }

    /// Set additional searchable text.
    #[must_use]
    pub fn search_text(mut self, value: impl Into<String>) -> Self {
        self.search_text = value.into();
        self
    }
}

/// Search hit from lexical overlay search.
#[derive(Debug, Clone)]
pub struct LexicalOverlaySearchHit {
    owner_path: String,
    selector: String,
    kind: String,
    name: String,
    search_text: String,
    score: f32,
    matched_terms: Vec<String>,
}

impl LexicalOverlaySearchHit {
    /// Owner path for the matched document.
    #[must_use]
    pub fn owner_path(&self) -> &str {
        &self.owner_path
    }

    /// Stable selector for the matched document.
    #[must_use]
    pub fn selector(&self) -> &str {
        &self.selector
    }

    /// Provider-neutral hit kind.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Display name for the hit.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Search text attached to the matched document.
    #[must_use]
    pub fn search_text(&self) -> &str {
        &self.search_text
    }

    /// Lexical overlay score.
    #[must_use]
    pub fn score(&self) -> f32 {
        self.score
    }

    /// Query terms matched by the overlay backend.
    #[must_use]
    pub fn matched_terms(&self) -> &[String] {
        &self.matched_terms
    }
}

/// Candidate hit projected from lexical overlay terms.
#[derive(Debug, Clone)]
pub struct LexicalOverlayCandidateHit {
    owner_path: String,
    symbol: String,
    text: String,
}

impl LexicalOverlayCandidateHit {
    /// Owner path for the matched candidate.
    #[must_use]
    pub fn owner_path(&self) -> &str {
        &self.owner_path
    }

    /// Original query term selected for this candidate.
    #[must_use]
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// Candidate text used by downstream graph/dependency extraction.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Run lexical overlay search without writing dirty evidence to durable DB.
#[must_use]
pub fn search_lexical_overlay(request: LexicalOverlaySearchRequest) -> LexicalOverlaySearchResult {
    let namespace = lexical_overlay_namespace(&request.source_snapshot);
    let source_snapshot = request.source_snapshot;
    let documents = request
        .documents
        .into_iter()
        .map(dynamic_overlay_document)
        .collect::<Vec<_>>();
    let mut overlay = default_dynamic_overlay_search_backend();
    overlay.upsert_documents(namespace.clone(), documents);
    let hits = overlay
        .search(
            &namespace,
            &DynamicOverlayQuery::new(request.query).limit(request.limit),
        )
        .into_iter()
        .map(lexical_overlay_hit)
        .collect();
    LexicalOverlaySearchResult {
        source_snapshot,
        hits,
    }
}

/// Project query terms to file-level lexical overlay candidates.
#[must_use]
pub fn search_lexical_overlay_candidates(
    terms: &[String],
    documents: &[LexicalOverlayDocument],
    source_snapshot: &agent_semantic_artifacts::SourceSnapshotEvidence,
    per_term_limit: usize,
    total_limit: usize,
) -> LexicalOverlayCandidateSearchResult {
    let mut remaining = total_limit;
    let mut candidates = Vec::new();
    let namespace = lexical_overlay_namespace(source_snapshot);
    let mut overlay = default_dynamic_overlay_search_backend();
    overlay.upsert_documents(
        namespace.clone(),
        documents
            .iter()
            .cloned()
            .map(dynamic_overlay_document)
            .collect(),
    );
    for term in terms {
        if remaining == 0 {
            break;
        }
        let hits = overlay.search(
            &namespace,
            &DynamicOverlayQuery::new(term).limit(per_term_limit),
        );
        for hit in hits {
            if remaining == 0 {
                break;
            }
            candidates.push(LexicalOverlayCandidateHit {
                owner_path: hit.document.owner_path,
                symbol: term.clone(),
                text: hit.document.search_text,
            });
            remaining -= 1;
        }
    }
    LexicalOverlayCandidateSearchResult {
        source_snapshot: source_snapshot.clone(),
        candidates,
    }
}

fn lexical_overlay_namespace(
    source_snapshot: &agent_semantic_artifacts::SourceSnapshotEvidence,
) -> DynamicOverlayNamespace {
    DynamicOverlayNamespace::new(
        "lexical-overlay",
        "workspace",
        "worktree",
        source_snapshot.provider_digest.clone(),
        source_snapshot.root_digest.clone(),
    )
}

fn dynamic_overlay_document(document: LexicalOverlayDocument) -> DynamicOverlayDocument {
    DynamicOverlayDocument {
        owner_path: document.owner_path,
        entity_id: document.selector.clone(),
        selector: document.selector,
        kind: document.kind,
        name: document.name,
        signature: None,
        display_range: None,
        source_hash: document.source_hash,
        search_text: document.search_text,
    }
}

fn lexical_overlay_hit(
    hit: crate::dynamic_overlay::DynamicOverlaySearchHit,
) -> LexicalOverlaySearchHit {
    LexicalOverlaySearchHit {
        owner_path: hit.document.owner_path,
        selector: hit.document.selector,
        kind: hit.document.kind,
        name: hit.document.name,
        search_text: hit.document.search_text,
        score: hit.score,
        matched_terms: hit.matched_terms,
    }
}
