#[path = "../../src/command/tree_sitter_query_diagnostics.rs"]
mod diagnostics;

#[test]
fn zero_match_guidance_explains_the_search_mode() {
    let output = diagnostics::render_search_miss_guidance("rust").join("\n");

    assert!(output.contains("mode: structural Tree-sitter reasoning search"));
    assert!(output.contains("no syntax capture satisfied"));
    assert!(output.contains("use `_` rather than `-`"));
    assert!(output.contains("asp rust search pipe"));
}

#[test]
fn successful_search_guidance_explains_compact_capture_rows() {
    assert!(diagnostics::render_search_match_guidance().contains("each `I=syntax:...` row"),);
}
