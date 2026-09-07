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
            "--selector",
            rust,
            "--selector",
            org,
        ])),
        Ok(ProgressiveQueryRequest::Selector {
            selectors: vec![org.to_owned(), rust.to_owned()],
            projection: "source".to_owned(),
            output_format: QueryOutputFormat::Human,
            workspace: None,
        })
    );
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
fn language_and_native_syntax_switches_remain_search_only() {
    let error = parse_progressive_query_args(&args(&[
        "query",
        "playbook",
        "--selector",
        "rust://src/lib.rs#item/function/run",
        "--languages",
        "rust",
    ]))
    .expect_err("selector scheme already owns provider routing");
    assert!(error.contains("query playbook does not support option `--languages`"));
}

#[test]
fn query_defaults_to_human_source_and_json_is_explicit() {
    let default = parse_progressive_query_args(&args(&[
        "query",
        "playbook",
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
