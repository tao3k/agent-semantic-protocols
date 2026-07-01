use crate::{
    LexicalOverlayDocument, LexicalOverlaySearchRequest, SourceIndexRankCandidate,
    lexical_overlay_hit_to_search_candidate, merge_search_candidates,
    merge_search_candidates_with_receipt, search_candidate_has_executable_line_identity,
    search_lexical_overlay, source_index_candidate_to_search_candidate, source_index_lookup_terms,
};
#[cfg(feature = "turso-overlay")]
use crate::{TursoStructuralIndexSearchHit, structural_index_hit_to_search_candidate};

#[test]
fn source_index_candidate_projects_to_shared_search_candidate_contract() {
    let terms = source_index_lookup_terms("source index fixture");
    let candidate = source_index_candidate_to_search_candidate(
        SourceIndexRankCandidate {
            ordinal: 0,
            path: "src/lib.rs".to_string(),
            query_keys: vec!["source_index_fixture".to_string(), "lib".to_string()],
        },
        &terms,
    );

    assert_eq!(candidate.route_source, "source-index");
    assert_eq!(candidate.fallback_reason, "none");
    assert_eq!(candidate.identity_kind, "owner-path");
    assert_eq!(candidate.owner_path.as_deref(), Some("src/lib.rs"));
    assert_eq!(candidate.field_hits[0].field, "query_keys");
    assert!(
        candidate.field_hits[0]
            .matched_terms
            .iter()
            .any(|term| term == "source")
    );
    assert!(!search_candidate_has_executable_line_identity(&candidate));
}

#[test]
fn lexical_overlay_hit_projects_selector_and_overlay_namespace() {
    let hits = search_lexical_overlay(
        LexicalOverlaySearchRequest::new("overlay fixture").document(
            LexicalOverlayDocument::new(
                "src/lib.rs",
                "rust://src/lib.rs#item/function/overlay_fixture",
                "overlay_fixture",
            )
            .search_text("dynamic overlay fixture owner"),
        ),
    );
    let candidate = lexical_overlay_hit_to_search_candidate(&hits[0], "session-1/base-1");

    assert_eq!(candidate.route_source, "dynamic-overlay");
    assert_eq!(candidate.fallback_reason, "none");
    assert_eq!(candidate.identity_kind, "selector");
    assert_eq!(
        candidate.selector.as_deref(),
        Some("rust://src/lib.rs#item/function/overlay_fixture")
    );
    assert_eq!(
        candidate.overlay_namespace.as_deref(),
        Some("session-1/base-1")
    );
    assert!(
        candidate
            .field_hits
            .iter()
            .any(|field| field.field == "search_text")
    );
    assert!(!search_candidate_has_executable_line_identity(&candidate));
}

#[cfg(feature = "turso-overlay")]
#[test]
fn structural_index_hit_projects_selector_generation_and_stable_route() {
    let terms = source_index_lookup_terms("parse config serde_json");
    let hit = TursoStructuralIndexSearchHit {
        document_id: "structural-index:generation-1:symbol:rust://src/lib.rs#item/fn/parse_config"
            .to_string(),
        selector: Some("rust://src/lib.rs#item/fn/parse_config".to_string()),
        document: "symbol parse_config rust rs-harness serde_json::from_str".to_string(),
    };
    let candidate = structural_index_hit_to_search_candidate(&hit, &terms);

    assert_eq!(candidate.route_source, "turso-fts");
    assert_eq!(candidate.fallback_reason, "none");
    assert_eq!(candidate.identity_kind, "selector");
    assert_eq!(candidate.generation.as_deref(), Some("generation-1"));
    assert_eq!(
        candidate.selector.as_deref(),
        Some("rust://src/lib.rs#item/fn/parse_config")
    );
    assert_eq!(candidate.field_hits[0].field, "structural_index_document");
    assert!(
        candidate.field_hits[0]
            .matched_terms
            .iter()
            .any(|term| term == "parse")
    );
    assert!(!search_candidate_has_executable_line_identity(&candidate));
}

#[test]
fn shared_search_candidate_detects_executable_line_identity() {
    let candidate = source_index_candidate_to_search_candidate(
        SourceIndexRankCandidate {
            ordinal: 0,
            path: "src/lib.rs:1:2".to_string(),
            query_keys: vec!["lib".to_string()],
        },
        &["lib".to_string()],
    );

    assert!(search_candidate_has_executable_line_identity(&candidate));
}

#[cfg(feature = "turso-overlay")]
#[test]
fn merge_search_candidates_prefers_overlay_then_structural_fts_then_source_index() {
    let terms = source_index_lookup_terms("overlay fixture");
    let source_index_candidate = source_index_candidate_to_search_candidate(
        SourceIndexRankCandidate {
            ordinal: 0,
            path: "src/lib.rs".to_string(),
            query_keys: vec!["overlay_fixture".to_string(), "lib".to_string()],
        },
        &terms,
    );
    let structural_candidate = structural_index_hit_to_search_candidate(
        &TursoStructuralIndexSearchHit {
            document_id:
                "structural-index:generation-1:symbol:rust://src/lib.rs#item/fn/overlay_fixture"
                    .to_string(),
            selector: Some("rust://src/lib.rs#item/fn/overlay_fixture".to_string()),
            document: "symbol overlay_fixture stable structural document".to_string(),
        },
        &terms,
    );
    let overlay_hits = search_lexical_overlay(
        LexicalOverlaySearchRequest::new("overlay fixture").document(
            LexicalOverlayDocument::new(
                "src/lib.rs",
                "rust://src/lib.rs#item/function/overlay_fixture",
                "overlay_fixture",
            )
            .search_text("dynamic overlay fixture owner"),
        ),
    );
    let overlay_candidate = lexical_overlay_hit_to_search_candidate(&overlay_hits[0], "session-1");

    let ranked = merge_search_candidates(vec![
        source_index_candidate,
        structural_candidate,
        overlay_candidate,
    ]);

    assert_eq!(ranked[0].candidate.route_source, "dynamic-overlay");
    assert_eq!(ranked[1].candidate.route_source, "turso-fts");
    assert_eq!(ranked[2].candidate.route_source, "source-index");
}

#[test]
fn merge_search_candidates_prefers_overlay_selector_then_stable_source_index() {
    let terms = source_index_lookup_terms("overlay fixture");
    let source_index_candidate = source_index_candidate_to_search_candidate(
        SourceIndexRankCandidate {
            ordinal: 0,
            path: "src/lib.rs".to_string(),
            query_keys: vec!["overlay_fixture".to_string(), "lib".to_string()],
        },
        &terms,
    );
    let overlay_hits = search_lexical_overlay(
        LexicalOverlaySearchRequest::new("overlay fixture").document(
            LexicalOverlayDocument::new(
                "src/lib.rs",
                "rust://src/lib.rs#item/function/overlay_fixture",
                "overlay_fixture",
            )
            .search_text("dynamic overlay fixture owner"),
        ),
    );
    let overlay_candidate = lexical_overlay_hit_to_search_candidate(&overlay_hits[0], "session-1");

    let ranked = merge_search_candidates(vec![source_index_candidate, overlay_candidate]);

    assert_eq!(ranked[0].candidate.route_source, "dynamic-overlay");
    assert_eq!(ranked[0].selector_bonus, 1);
    assert_eq!(ranked[1].candidate.route_source, "source-index");
}

#[test]
fn merge_search_candidates_filters_line_range_identity() {
    let line_range_candidate = source_index_candidate_to_search_candidate(
        SourceIndexRankCandidate {
            ordinal: 0,
            path: "src/lib.rs:1:2".to_string(),
            query_keys: vec!["lib".to_string()],
        },
        &["lib".to_string()],
    );

    assert!(merge_search_candidates(vec![line_range_candidate]).is_empty());
}

#[test]
fn merge_search_candidates_with_receipt_records_stage_counts_and_fallback_reason() {
    let stable_candidate = source_index_candidate_to_search_candidate(
        SourceIndexRankCandidate {
            ordinal: 0,
            path: "src/lib.rs".to_string(),
            query_keys: vec!["lib".to_string()],
        },
        &["lib".to_string()],
    );
    let line_range_candidate = source_index_candidate_to_search_candidate(
        SourceIndexRankCandidate {
            ordinal: 1,
            path: "src/generated.rs:1:2".to_string(),
            query_keys: vec!["generated".to_string()],
        },
        &["generated".to_string()],
    );

    let receipt =
        merge_search_candidates_with_receipt(vec![stable_candidate, line_range_candidate]);

    assert_eq!(receipt.ranked.len(), 1);
    assert_eq!(receipt.stage.stage, "search-candidate-merge");
    assert_eq!(receipt.stage.candidate_count, 2);
    assert_eq!(receipt.stage.returned_count, 1);
    assert_eq!(receipt.stage.filtered_line_identity_count, 1);
    assert_eq!(receipt.stage.fallback_reason, "none");
    assert_eq!(
        receipt.stage.route_sources,
        vec!["source-index".to_string()]
    );
}

#[test]
fn merge_search_candidates_with_receipt_reports_line_identity_filtered_fallback() {
    let line_range_candidate = source_index_candidate_to_search_candidate(
        SourceIndexRankCandidate {
            ordinal: 0,
            path: "src/lib.rs:1:2".to_string(),
            query_keys: vec!["lib".to_string()],
        },
        &["lib".to_string()],
    );

    let receipt = merge_search_candidates_with_receipt(vec![line_range_candidate]);

    assert!(receipt.ranked.is_empty());
    assert_eq!(receipt.stage.candidate_count, 1);
    assert_eq!(receipt.stage.returned_count, 0);
    assert_eq!(receipt.stage.filtered_line_identity_count, 1);
    assert_eq!(receipt.stage.fallback_reason, "line-identity-filtered");
}
