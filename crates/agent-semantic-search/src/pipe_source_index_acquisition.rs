//! Snapshot-keyed source-index acquisition for the search pipe.

use std::path::{Path, PathBuf};

use crate::SearchPipeCandidate;

macro_rules! source_index_acquisition_text {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl From<&String> for $name {
            fn from(value: &String) -> Self {
                Self(value.clone())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

source_index_acquisition_text!(SearchPipeSourceIndexPath);
source_index_acquisition_text!(SearchPipeSourceIndexLanguageId);
source_index_acquisition_text!(SearchPipeSourceIndexProviderId);
source_index_acquisition_text!(SearchPipeSourceIndexSourceKind);
source_index_acquisition_text!(SearchPipeSourceIndexQueryKey);
source_index_acquisition_text!(SearchPipeSourceIndexLookupState);
source_index_acquisition_text!(SearchPipeSourceIndexArtifactDigest);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceIndexCandidate {
    pub path: SearchPipeSourceIndexPath,
    pub language_id: Option<SearchPipeSourceIndexLanguageId>,
    pub provider_id: Option<SearchPipeSourceIndexProviderId>,
    pub source_kind: SearchPipeSourceIndexSourceKind,
    pub line_count: Option<u32>,
    pub query_keys: Vec<SearchPipeSourceIndexQueryKey>,
    pub selector_projection:
        Option<agent_semantic_content_identity::ExactSelectorProjectionRecordV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceIndexLookup {
    pub state: SearchPipeSourceIndexLookupState,
    pub candidates: Vec<SearchPipeSourceIndexCandidate>,
    pub source_snapshot: Option<agent_semantic_content_identity::SourceSnapshotEvidence>,
    pub index_artifact_digest: Option<SearchPipeSourceIndexArtifactDigest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceIndexGate {
    pub term_count: usize,
    pub generic_term_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchPipeSourceIndexDecision {
    QueryGate,
    DeferBackend,
    UseAndSkipSearchOverlay,
    GenerationUnavailable,
    Fallthrough,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceIndexAcquisition {
    pub decision: SearchPipeSourceIndexDecision,
    pub gate: Option<SearchPipeSourceIndexGate>,
    pub(crate) candidates: Vec<SearchPipeCandidate>,
    pub source_snapshot: Option<agent_semantic_content_identity::SourceSnapshotEvidence>,
    /// Canonical generation authority for every graph-derived candidate.
    ///
    /// A source snapshot alone is not sufficient: filesystem/search overlays
    /// can describe only a package fragment while still carrying a valid
    /// snapshot digest. Graph routing must bind its facts to the complete,
    /// active workspace generation before it may rank or render them.
    pub workspace_generation:
        agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationAuthority,
    pub index_artifact_digest: Option<String>,
}

impl SearchPipeSourceIndexAcquisition {
    /// Bind a validated complete generation to this acquisition.
    pub fn bind_workspace_generation(
        &mut self,
        evidence: agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
    ) -> Result<
        (),
        agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceError,
    >{
        evidence.validate_complete()?;
        self.workspace_generation =
            agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationAuthority::Active {
                evidence,
            };
        Ok(())
    }

    /// Untrusted discovery hints may be inspected without graph authority.
    pub fn discovery_candidates(&self) -> &[SearchPipeCandidate] {
        &self.candidates
    }

    /// Graph consumers must prove that this acquisition belongs to the active
    /// complete workspace generation before observing candidates.
    pub fn admitted_graph_candidates(
        &self,
        active: &agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
    ) -> Result<
        &[SearchPipeCandidate],
        agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceError,
    >{
        self.workspace_generation.admit_graph(active)?;
        Ok(&self.candidates)
    }
}

pub struct SearchPipeSourceIndexAcquisitionRequest<'a> {
    pub intent: &'a str,
    pub project_root: &'a Path,
    pub scopes: &'a [PathBuf],
    pub lookup: Option<&'a SearchPipeSourceIndexLookup>,
}

pub fn collect_search_pipe_source_index_acquisition(
    request: SearchPipeSourceIndexAcquisitionRequest<'_>,
) -> Option<SearchPipeSourceIndexAcquisition> {
    let SearchPipeSourceIndexAcquisitionRequest {
        intent,
        project_root,
        scopes,
        lookup,
    } = request;
    if !scopes.is_empty() {
        return None;
    }
    Some(collect_admitted_source_index_acquisition(
        intent,
        project_root,
        lookup?,
    ))
}

fn collect_admitted_source_index_acquisition(
    intent: &str,
    project_root: &Path,
    lookup: &SearchPipeSourceIndexLookup,
) -> SearchPipeSourceIndexAcquisition {
    let candidates = lookup
        .candidates
        .iter()
        .map(|candidate| {
            crate::pipe_source_index_projection::source_index_candidate(
                project_root,
                intent,
                candidate,
            )
        })
        .collect::<Vec<_>>();
    let decision = if lookup.state == "generation-unavailable".into() && candidates.is_empty() {
        SearchPipeSourceIndexDecision::GenerationUnavailable
    } else if intent_terms_all_path_like(intent) && lookup.state.as_str() == "miss" {
        SearchPipeSourceIndexDecision::DeferBackend
    } else if candidates.is_empty() {
        SearchPipeSourceIndexDecision::Fallthrough
    } else if candidates
        .iter()
        .all(crate::pipe_source_index_projection::source_index_candidate_ready)
    {
        SearchPipeSourceIndexDecision::UseAndSkipSearchOverlay
    } else {
        SearchPipeSourceIndexDecision::DeferBackend
    };
    SearchPipeSourceIndexAcquisition {
        workspace_generation: agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationAuthority::unavailable(),
        decision,
        gate: None,
        candidates,
        source_snapshot: lookup.source_snapshot.clone(),
        index_artifact_digest: (lookup.index_artifact_digest.clone())
            .map(|value| value.to_string()),
    }
}

#[must_use]
pub fn search_pipe_source_index_query_gate(
    terms: &[crate::SearchPipeQueryTerm],
) -> Option<SearchPipeSourceIndexGate> {
    if terms.len() < 2
        || terms.iter().any(|term| {
            matches!(
                term.role,
                crate::SearchPipeTermRole::Symbol
                    | crate::SearchPipeTermRole::Literal
                    | crate::SearchPipeTermRole::DiagnosticCode
            ) || crate::search_pipe_is_path_like_token(&term.raw)
        })
    {
        return None;
    }
    Some(SearchPipeSourceIndexGate {
        term_count: terms.len(),
        generic_term_count: terms
            .iter()
            .filter(|term| term.role != crate::SearchPipeTermRole::Symbol)
            .count(),
    })
}

fn intent_terms_all_path_like(intent: &str) -> bool {
    let terms = intent
        .split(|character: char| character == ',' || character == '|' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    !terms.is_empty()
        && terms
            .iter()
            .all(|term| term.contains('/') || term.contains('\\') || term.contains('.'))
}
