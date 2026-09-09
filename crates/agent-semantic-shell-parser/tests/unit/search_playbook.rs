// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_shell_parser::{
    SearchPlaybookBlockKind, SearchPlaybookIssueKind, parse_search_playbook_boundaries,
};

fn argv(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn native_rg_argv_is_retained_without_interpretation() {
    let args = argv(&[
        "search",
        "playbook",
        "--language",
        "rust",
        "--rg",
        "-n",
        "-g",
        "*.rs",
        "--type",
        "rust",
        "needle",
        "crates",
        "--tantivy",
        "title:\"needle owner\"^2 OR body:route",
        "--graph",
        "gql",
        "MATCH (a)-[:CALLS]->(b) RETURN a,b",
    ]);
    let parsed = parse_search_playbook_boundaries(&args);
    assert!(parsed.is_valid(), "{:?}", parsed.issues);
    let rg = parsed
        .blocks
        .iter()
        .find(|block| block.kind == SearchPlaybookBlockKind::Rg)
        .expect("rg block");
    assert_eq!(
        rg.argv,
        argv(&["-n", "-g", "*.rs", "--type", "rust", "needle", "crates"])
    );
    assert_eq!(
        &args[rg.argv_start_token_index..rg.argv_end_token_index_exclusive],
        rg.argv
    );
}

#[test]
fn filename_scoped_rg_and_full_corpus_tantivy_form_an_admitted_pair() {
    let parsed = parse_search_playbook_boundaries(&argv(&[
        "search",
        "playbook",
        "--language",
        "rust",
        "--rg",
        "--files",
        "-g",
        "src/lib.rs",
        "--tantivy",
        "title:[* TO *] OR body:[* TO *]",
    ]));
    assert!(parsed.is_valid(), "{:?}", parsed.issues);
}

#[test]
fn rg_option_values_that_look_like_playbook_boundaries_remain_native_argv() {
    let args = argv(&[
        "search",
        "playbook",
        "--language",
        "rust",
        "--rg",
        "-n",
        "-e",
        "--graph",
        "--replace",
        "--syntax",
        ".",
        "--tantivy",
        "title:\"--graph owner\"^2 OR body:syntax",
    ]);
    let parsed = parse_search_playbook_boundaries(&args);
    assert!(parsed.is_valid(), "{:?}", parsed.issues);
    let rg = parsed
        .blocks
        .iter()
        .find(|block| block.kind == SearchPlaybookBlockKind::Rg)
        .expect("rg block");
    assert_eq!(
        rg.argv,
        argv(&["-n", "-e", "--graph", "--replace", "--syntax", "."])
    );
}

#[test]
fn native_rg_double_dash_preserves_a_reserved_looking_pattern() {
    let args = argv(&[
        "search",
        "playbook",
        "--language",
        "rust",
        "--rg",
        "--",
        "--graph",
        ".",
        "--tantivy",
        "title:\"graph option\"^2 OR body:boundary",
    ]);
    let parsed = parse_search_playbook_boundaries(&args);
    assert!(parsed.is_valid(), "{:?}", parsed.issues);
    let rg = parsed
        .blocks
        .iter()
        .find(|block| block.kind == SearchPlaybookBlockKind::Rg)
        .expect("rg block");
    assert_eq!(rg.argv, argv(&["--", "--graph", "."]));
}

#[test]
fn optional_calibration_fields_are_order_independent() {
    let parsed = parse_search_playbook_boundaries(&argv(&[
        "search",
        "playbook",
        "--rg",
        "needle",
        ".",
        "--tantivy",
        "title:\"needle owner\"^2 OR body:route",
        "--language",
        "rust",
    ]));
    assert!(parsed.is_valid(), "{:?}", parsed.issues);
}

#[test]
fn typed_cli_values_are_validated_before_runtime_dispatch() {
    for (args, expected, token_index) in [
        (
            argv(&[
                "search",
                "playbook",
                "--language",
                "rust",
                "--workspace",
                "../other",
                "--syntax",
                "rust",
                "query",
            ]),
            SearchPlaybookIssueKind::InvalidWorkspaceIdentity,
            5,
        ),
        (
            argv(&[
                "search",
                "playbook",
                "--language",
                "rust",
                "--syntax",
                "rust|python",
                "query",
            ]),
            SearchPlaybookIssueKind::InvalidSyntaxProducer,
            5,
        ),
        (
            argv(&[
                "search",
                "playbook",
                "--language",
                "rust",
                "--native-syntax",
                "src/lib.rs:10",
            ]),
            SearchPlaybookIssueKind::InvalidNativeSelector,
            5,
        ),
        (
            argv(&[
                "search",
                "playbook",
                "--language",
                "rust",
                "--syntax",
                "rust",
                "query",
                "--graph",
                "cypher",
                "MATCH (a)-[:CALLS]->(b) RETURN a,b",
            ]),
            SearchPlaybookIssueKind::InvalidGraphLanguage,
            8,
        ),
    ] {
        let parsed = parse_search_playbook_boundaries(&args);
        assert!(
            parsed
                .issues
                .iter()
                .any(|issue| { issue.kind == expected && issue.token_index == Some(token_index) })
        );
    }
}

#[test]
fn missing_pair_is_one_structured_layout_issue() {
    let parsed = parse_search_playbook_boundaries(&argv(&[
        "search",
        "playbook",
        "--language",
        "rust",
        "--rg",
        "needle",
        ".",
    ]));
    assert_eq!(parsed.missing_fields, vec!["tantivy"]);
    assert!(parsed.issues.iter().any(|issue| {
        issue.kind == SearchPlaybookIssueKind::PairedInputRequired && issue.field == "tantivy"
    }));
}

#[test]
fn unknown_option_and_graph_barrier_report_exact_locations() {
    let parsed = parse_search_playbook_boundaries(&argv(&[
        "search",
        "playbook",
        "--language",
        "rust",
        "--bogus",
        "src",
        "--graph",
        "gql",
        "MATCH (n) RETURN n",
        "--syntax",
        "rust",
        "query",
    ]));
    assert!(parsed.issues.iter().any(|issue| {
        issue.kind == SearchPlaybookIssueKind::UnknownOption && issue.token_index == Some(4)
    }));
    assert!(parsed.issues.iter().any(|issue| {
        issue.kind == SearchPlaybookIssueKind::InputAfterGraph && issue.token_index == Some(9)
    }));
}

#[test]
fn syntax_and_native_syntax_remain_legal_inputs() {
    for args in [
        argv(&[
            "search",
            "playbook",
            "--rg",
            "main",
            ".",
            "--tantivy",
            "title:\"main function\"^2 OR body:main",
            "--syntax",
            "rust",
            "query",
        ]),
        argv(&[
            "search",
            "playbook",
            "--rg",
            "main",
            ".",
            "--tantivy",
            "title:\"main function\"^2 OR body:main",
            "--native-syntax",
            "rust://src/lib.rs#item/function/main",
        ]),
    ] {
        let parsed = parse_search_playbook_boundaries(&args);
        assert!(parsed.is_valid(), "{:?}", parsed.issues);
    }
}

#[test]
fn bare_tantivy_text_is_rejected_with_structural_requirements() {
    let parsed = parse_search_playbook_boundaries(&argv(&[
        "search",
        "playbook",
        "--language",
        "rust",
        "--rg",
        "needle",
        ".",
        "--tantivy",
        "needle",
    ]));
    let analysis = parsed.tantivy_analyses.first().expect("Tantivy analysis");
    assert_eq!(
        analysis.missing_features,
        vec![
            "multiple-clauses",
            "fielded-clause",
            "explicit-boolean-composition",
            "phrase-boost-or-structured-predicate",
        ]
    );
    assert!(
        parsed
            .issues
            .iter()
            .any(|issue| { issue.kind == SearchPlaybookIssueKind::TantivyExpressionTooSimple })
    );
}
