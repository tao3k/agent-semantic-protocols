// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{ProgressiveQueryRequest, QueryOutputFormat, parse_progressive_query_args};

fn query(source: &str) -> Result<ProgressiveQueryRequest, String> {
    parse_progressive_query_args(&["query".to_owned(), "playbook".to_owned(), source.to_owned()])
}

#[test]
fn query_playbook_lowers_one_canonical_polyglot_scheme_expression() {
    let rust =
        "rust://src/registry.rs#item/method/refresh/scope/implementation-owner/type/Registry";
    let org = "org://docs/publication.org#item/heading/Publication";
    let source = format!(
        "(query (producers (language rust) (documents org)) (select (selectors {rust:?} {org:?})))"
    );
    assert_eq!(
        query(&source),
        Ok(ProgressiveQueryRequest::Selector {
            language: Some("rust".to_owned()),
            documents: Some("org".to_owned()),
            selectors: vec![rust.to_owned(), org.to_owned()],
            projection: "source".to_owned(),
            output_format: QueryOutputFormat::Human,
            workspace: None,
        })
    );
}

#[test]
fn query_playbook_preserves_order_and_all_explicit_controls_in_scheme() {
    let rust = "rust://src/lib.rs#item/function/run";
    let json = "json://schemas/config.schema.json#item/pointer/properties/hook";
    let source = format!(
        "(query (workspace \"registered\") (producers (language rust) (documents json)) (select (selectors {rust:?} {json:?}) (projection callable-skeleton) (output json)))"
    );
    assert_eq!(
        query(&source),
        Ok(ProgressiveQueryRequest::Selector {
            language: Some("rust".to_owned()),
            documents: Some("json".to_owned()),
            selectors: vec![rust.to_owned(), json.to_owned()],
            projection: "callable-skeleton".to_owned(),
            output_format: QueryOutputFormat::Json,
            workspace: Some("registered".to_owned()),
        })
    );
}

#[test]
fn query_rejects_flags_instead_of_retaining_a_compatibility_grammar() {
    let error = parse_progressive_query_args(&[
        "query".to_owned(),
        "playbook".to_owned(),
        "--language".to_owned(),
        "rust".to_owned(),
    ])
    .expect_err("flags are not a second public grammar");
    assert!(error.contains("exactly one Scheme expression"));
}

#[test]
fn markerless_query_is_rejected_instead_of_creating_a_second_public_grammar() {
    let error = parse_progressive_query_args(&[
        "query".to_owned(),
        "(query (producers (language rust)) (select (selectors \"rust://src/lib.rs#item/function/run\")))".to_owned(),
    ])
    .expect_err("Query Playbook marker is mandatory");
    assert!(error.contains("use `asp query playbook`"));
}

#[test]
fn query_rejects_a_selector_outside_the_declared_producer_sets() {
    let error = query(
        "(query (producers (language python)) (select (selectors \"rust://src/lib.rs#item/function/run\")))",
    )
    .expect_err("selector producer must agree with Scheme producers");
    assert!(error.contains("declared producer set"));
}

#[test]
fn query_rejects_duplicate_selectors_and_unknown_operators() {
    let duplicate = query(
        "(query (producers (language rust)) (select (selectors \"rust://src/lib.rs#item/function/run\" \"rust://src/lib.rs#item/function/run\")))",
    )
    .expect_err("selector identity set must be unique");
    assert!(duplicate.contains("selectors must be unique"));

    let unknown = query(
        "(query (producers (language rust)) (select (selectors \"rust://src/lib.rs#item/function/run\") (jq \".name\")))",
    )
    .expect_err("unknown Scheme operator");
    assert!(unknown.contains("does not support operator `jq`"));
}
