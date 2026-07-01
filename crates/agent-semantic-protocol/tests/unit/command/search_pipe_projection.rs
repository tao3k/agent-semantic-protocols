mod document_provider {
    pub(super) fn is_document_language(language_id: &str) -> bool {
        matches!(language_id, "md" | "org")
    }
}

mod search_pipe_model {
    pub(super) struct Candidate {
        pub(super) path: String,
        pub(super) line: usize,
        pub(super) end_line: usize,
        pub(super) selector: Option<String>,
    }
}

#[path = "../../../src/command/search_pipe_projection.rs"]
mod search_pipe_projection;

use search_pipe_model::Candidate;

#[test]
fn candidate_selector_prefers_provider_selector_over_line_range_hint() {
    let candidate = Candidate {
        path: "src/lib.rs".to_string(),
        line: 42,
        end_line: 47,
        selector: Some("rust://src/lib.rs#item/fn/parse_config".to_string()),
    };

    assert_eq!(
        search_pipe_projection::candidate_selector("rust", &candidate),
        "rust://src/lib.rs#item/fn/parse_config"
    );
}

#[test]
fn query_projection_uses_content_mode_for_document_languages() {
    assert_eq!(
        search_pipe_projection::query_projection_kind("org"),
        "content"
    );
    assert_eq!(
        search_pipe_projection::query_projection_flag("org"),
        "--content"
    );
}

#[test]
fn query_projection_uses_code_mode_for_source_languages() {
    assert_eq!(
        search_pipe_projection::query_projection_kind("rust"),
        "code"
    );
    assert_eq!(
        search_pipe_projection::query_projection_flag("rust"),
        "--code"
    );
}
