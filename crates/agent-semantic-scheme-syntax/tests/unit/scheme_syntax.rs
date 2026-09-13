// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_scheme_syntax::{
    SCHEME_GRAMMAR_ID, SchemeDatum, admit_scheme_source, parse_scheme_datums,
};

#[test]
fn admission_reports_canonical_grammar_and_top_level_heads() {
    let source = "; ignored\n(search (rg \"owner\" \".\"))\n(query (owner rust))";
    let admission = admit_scheme_source(source).expect("valid Scheme source");
    assert_eq!(admission.grammar_id, SCHEME_GRAMMAR_ID);
    assert_eq!(admission.top_level_form_count, 2);
    assert_eq!(admission.top_level_form_heads, ["search", "query"]);
}

#[test]
fn comments_only_source_is_rejected_as_empty() {
    let error = admit_scheme_source("; no executable datum\n")
        .expect_err("comments are not executable Scheme source");
    assert_eq!(error.reason_kind, "scheme-source-empty");
}

#[test]
fn incomplete_source_preserves_tree_sitter_location() {
    let error =
        admit_scheme_source("(search (rg \"owner\"").expect_err("unclosed list must fail closed");
    assert!(matches!(
        error.reason_kind,
        "scheme-source-invalid-syntax" | "scheme-source-missing-syntax"
    ));
    assert!(error.to_string().contains("at byte"));
}

#[test]
fn datum_lowering_preserves_nested_lists_and_string_escapes() {
    let datums = parse_scheme_datums(r#"(search (rg "line\nvalue" "."))"#)
        .expect("supported Scheme datum subset");
    assert_eq!(
        datums,
        [SchemeDatum::List(vec![
            SchemeDatum::Symbol("search".to_owned()),
            SchemeDatum::List(vec![
                SchemeDatum::Symbol("rg".to_owned()),
                SchemeDatum::String("line\nvalue".to_owned()),
                SchemeDatum::String(".".to_owned()),
            ]),
        ])]
    );
}
