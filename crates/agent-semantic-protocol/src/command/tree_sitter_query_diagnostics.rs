pub(crate) fn render_query_summary(
    language_id: &str,
    total_captures: usize,
    retained_captures: usize,
) -> String {
    let status = if total_captures == 0 {
        "no-matches"
    } else {
        "matches"
    };
    format!(
        "[query-treesitter] status={status} language={language_id} matches={total_captures} retained={retained_captures} truncated={}",
        total_captures > retained_captures
    )
}

pub(crate) fn render_query_miss_guidance(language_id: &str) -> Vec<String> {
    let identifier_hint = if language_id == "rust" {
        "hint: a value compared with an identifier capture must be a valid Rust identifier; use `_` rather than `-` inside an identifier."
    } else {
        "hint: a value compared with an identifier capture must be a valid identifier for this language."
    };
    vec![
        "mode: structural Tree-sitter query; predicates compare captured source text exactly."
            .to_string(),
        "result: no syntax capture satisfied the complete pattern and its predicates.".to_string(),
        identifier_hint.to_string(),
        format!(
            "next: use `asp {language_id} search pipe '<symbol-or-concept>' --workspace . --view seeds` for discovery; use `query` after the syntax shape is known."
        ),
    ]
}

pub(crate) fn render_query_match_guidance() -> &'static str {
    "result: each `I=syntax:...` row is one retained capture; source code and capture text are intentionally omitted."
}
