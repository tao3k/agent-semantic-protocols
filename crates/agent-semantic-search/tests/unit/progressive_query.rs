// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{ProgressiveQueryRequest, QueryOutputFormat, parse_progressive_query_args};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn query_playbook_collects_one_canonical_polyglot_selector_set() {
    let rust =
        "rust://src/registry.rs#item/method/refresh/scope/implementation-owner/type/Registry";
    let org = "org://docs/publication.org#item/heading/Publication";
    assert_eq!(
        parse_progressive_query_args(&args(&[
            "query",
            "playbook",
            "--language",
            "rust",
            "--documents",
            "org",
            "--selector",
            rust,
            "--selector",
            org,
        ])),
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
fn query_playbook_preserves_request_order_and_does_not_invent_a_jq_binding_rule() {
    let rust = "rust://src/lib.rs#item/function/run";
    let json = "json://schemas/config.schema.json#item/pointer/properties/hook";
    let request = parse_progressive_query_args(&args(&[
        "query",
        "playbook",
            "--language",
            "rust",
            "--documents",
            "json",
        "--selector",
        rust,
        "--selector",
        json,
    ]))
    .expect("ordered selector request");
    assert!(matches!(
        request,
        ProgressiveQueryRequest::Selector { selectors, .. }
            if selectors == vec![rust.to_owned(), json.to_owned()]
    ));

    let error = parse_progressive_query_args(&args(&[
        "query",
        "playbook",
        "--language",
        "json",
        "--selector",
        json,
        "--jq",
        ".properties.hook",
    ]))
    .expect_err("jq input binding has no shared Query Playbook contract yet");
    assert!(error.contains("does not support option `--jq`"));
}

#[test]
fn markerless_query_is_rejected_instead_of_creating_a_second_public_grammar() {
    let error = parse_progressive_query_args(&args(&[
        "query",
        "--selector",
        "rust://src/lib.rs#item/function/run",
    ]))
    .expect_err("Query Playbook marker is mandatory");
    assert!(error.contains("use `asp query playbook`"));
}

#[test]
fn query_rejects_a_selector_outside_the_declared_producer_sets() {
    let error = parse_progressive_query_args(&args(&[
        "query",
        "playbook",
        "--language",
        "python",
        "--selector",
        "rust://src/lib.rs#item/function/run",
    ]))
    .expect_err("selector producer must agree with --language");
    assert!(error.contains("declared --language/--documents producer set"));
}

#[test]
fn query_defaults_to_human_source_and_json_is_explicit() {
    let default = parse_progressive_query_args(&args(&[
        "query",
        "playbook",
        "--language",
        "gerbil-scheme",
        "--selector",
        "gerbil-scheme://src/build-api/package-spec.ss#item/function/package-spec",
    ]))
    .expect("default exact query");
    assert!(matches!(
        default,
        ProgressiveQueryRequest::Selector {
            selectors,
            projection,
            output_format: QueryOutputFormat::Human,
            ..
        } if projection == "source" && selectors.len() == 1
    ));

    let json = parse_progressive_query_args(&args(&[
        "query",
        "playbook",
        "--language",
        "gerbil-scheme",
        "--selector",
        "gerbil-scheme://src/build-api/package-spec.ss#item/function/package-spec",
        "--json",
    ]))
    .expect("explicit machine query");
    assert!(matches!(
        json,
        ProgressiveQueryRequest::Selector {
            output_format: QueryOutputFormat::Json,
            ..
        }
    ));
}
