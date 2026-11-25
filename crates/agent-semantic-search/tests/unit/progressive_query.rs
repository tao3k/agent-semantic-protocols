// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    ProgressiveQueryInput, ProgressiveQueryRequest, QueryOutputFormat, admit_progressive_query,
};

#[test]
fn typed_query_admits_canonical_polyglot_selectors() {
    let rust =
        "rust://src/registry.rs#item/method/refresh/scope/implementation-owner/type/Registry";
    let org = "org://docs/publication.org#item/heading/Publication";
    let request = admit_progressive_query(ProgressiveQueryInput {
        workspace: None,
        languages: vec!["rust".to_owned()],
        documents: vec!["org".to_owned()],
        selectors: vec![rust.to_owned(), org.to_owned()],
        projection: None,
        output_format: None,
    })
    .expect("valid polyglot query");
    assert_eq!(
        request,
        ProgressiveQueryRequest::Selector {
            language: Some("rust".to_owned()),
            documents: Some("org".to_owned()),
            selectors: vec![rust.to_owned(), org.to_owned()],
            projection: "source".to_owned(),
            output_format: QueryOutputFormat::Human,
            workspace: None,
        }
    );
}

#[test]
fn typed_query_preserves_explicit_controls_and_rejects_drift() {
    let rust = "rust://src/lib.rs#item/function/run";
    let request = admit_progressive_query(ProgressiveQueryInput {
        workspace: Some("marlin-core".to_owned()),
        languages: vec!["rust".to_owned()],
        documents: vec![],
        selectors: vec![rust.to_owned()],
        projection: Some("callable-skeleton".to_owned()),
        output_format: Some(QueryOutputFormat::Json),
    })
    .expect("valid explicit controls");
    assert_eq!(
        request,
        ProgressiveQueryRequest::Selector {
            language: Some("rust".to_owned()),
            documents: None,
            selectors: vec![rust.to_owned()],
            projection: "callable-skeleton".to_owned(),
            output_format: QueryOutputFormat::Json,
            workspace: Some("marlin-core".to_owned()),
        }
    );

    let out_of_scope = admit_progressive_query(ProgressiveQueryInput {
        workspace: None,
        languages: vec!["python".to_owned()],
        documents: vec![],
        selectors: vec![rust.to_owned()],
        projection: None,
        output_format: None,
    })
    .expect_err("undeclared producer must fail");
    assert!(out_of_scope.contains("declared producer set"));

    let duplicate = admit_progressive_query(ProgressiveQueryInput {
        workspace: None,
        languages: vec!["rust".to_owned()],
        documents: vec![],
        selectors: vec![rust.to_owned(), rust.to_owned()],
        projection: None,
        output_format: None,
    })
    .expect_err("duplicate selectors must fail");
    assert!(duplicate.contains("selectors must be unique"));
}
