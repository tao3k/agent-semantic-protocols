#[path = "../../src/command/tree_sitter_query_diagnostics.rs"]
mod diagnostics;

#[test]
fn zero_match_summary_is_single_and_explains_the_search_mode() {
    let summary = diagnostics::render_search_summary("rust", 0, 0);
    let guidance = diagnostics::render_search_miss_guidance("rust");
    let output = std::iter::once(summary)
        .chain(guidance)
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(output.matches("[search-treesitter]").count(), 1);
    assert!(output.contains("status=no-matches"));
    assert!(output.contains("mode: structural Tree-sitter reasoning search"));
    assert!(output.contains("no syntax capture satisfied"));
    assert!(output.contains("use `_` rather than `-`"));
    assert!(output.contains("asp rust search pipe"));
}

#[test]
fn successful_summary_explains_compact_capture_rows() {
    let summary = diagnostics::render_search_summary("rust", 3, 2);

    assert!(summary.contains("status=matches"));
    assert!(summary.contains("matches=3"));
    assert!(summary.contains("retained=2"));
    assert!(summary.contains("truncated=true"));
    assert!(diagnostics::render_search_match_guidance().contains("each `I=syntax:...` row"),);
}
