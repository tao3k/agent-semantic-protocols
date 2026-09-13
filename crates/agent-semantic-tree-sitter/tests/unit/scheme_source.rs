// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::{SchemeDatum, admit_scheme_source, parse_scheme_datums};

#[test]
fn admission_records_tree_sitter_provenance_and_form_heads() {
    let receipt = admit_scheme_source(
        "; declaration\n(search (producers (language rust)) (rg \"owner\" \".\"))",
    )
    .expect("admit structurally complete Scheme");
    assert_eq!(receipt.grammar_id, "tree-sitter-scheme");
    assert_eq!(receipt.grammar_version, "0.24.7");
    assert_eq!(receipt.top_level_form_count, 1);
    assert_eq!(receipt.top_level_form_heads, ["search"]);
}

#[test]
fn datum_projection_decodes_strings_without_evaluating_scheme() {
    let datums = parse_scheme_datums(r#"(search (rg "-e" "owner\nidentity" "."))"#)
        .expect("parse one supported datum");
    assert_eq!(
        datums,
        vec![SchemeDatum::List(vec![
            SchemeDatum::Symbol("search".to_owned()),
            SchemeDatum::List(vec![
                SchemeDatum::Symbol("rg".to_owned()),
                SchemeDatum::String("-e".to_owned()),
                SchemeDatum::String("owner\nidentity".to_owned()),
                SchemeDatum::String(".".to_owned()),
            ]),
        ])]
    );
}

#[test]
fn unsupported_reader_forms_fail_closed() {
    let error = parse_scheme_datums("'(search)").expect_err("quote is not a data-plan form");
    assert_eq!(error.reason_kind, "scheme-datum-unsupported");
}

#[test]
fn incomplete_source_reports_a_location() {
    let error = admit_scheme_source("(search").expect_err("missing close delimiter");
    assert!(matches!(
        error.reason_kind,
        "scheme-source-invalid-syntax" | "scheme-source-missing-syntax"
    ));
    assert!(error.byte_offset <= "(search".len());
}
