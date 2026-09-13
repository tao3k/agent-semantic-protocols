// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{NativeRgDiagnosticKind, NativeRgOutputAttribution, analyze_native_rg_argv};

fn argv(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn complex_native_argv_is_parsed_without_rewrite() {
    let exact = argv(&[
        "-nFi",
        "-g*.rs",
        "--type",
        "rust",
        "-e",
        "Owner|Route",
        "crates",
    ]);
    let analysis = analyze_native_rg_argv(&exact);
    assert!(analysis.is_admitted(), "{:?}", analysis.diagnostics);
    assert_eq!(analysis.argv, exact);
    assert_eq!(analysis.patterns[0].value, "Owner|Route");
    assert_eq!(analysis.search_roots[0].value, "crates");
    assert_eq!(
        analysis.output_attribution,
        NativeRgOutputAttribution::PathLine
    );
    assert!(
        analysis
            .options
            .iter()
            .any(|option| { option.option == "-g" && option.value.as_deref() == Some("*.rs") })
    );
}

#[test]
fn missing_option_value_is_located_before_execution() {
    let analysis = analyze_native_rg_argv(&argv(&["-n", "--glob"]));
    assert!(analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == NativeRgDiagnosticKind::MissingOptionValue
            && diagnostic.argv_token_index == Some(1)
    }));
}

#[test]
fn preprocessor_and_external_roots_fail_the_generation_boundary() {
    for exact in [
        argv(&["--pre", "cat", "needle", "."]),
        argv(&["needle", "../other"]),
        argv(&["-f", "/tmp/patterns", "."]),
    ] {
        let analysis = analyze_native_rg_argv(&exact);
        assert!(!analysis.is_admitted());
    }
}

#[test]
fn filename_only_is_an_attributable_owner_scope() {
    let analysis = analyze_native_rg_argv(&argv(&["-l", "needle", "."]));
    assert!(analysis.is_admitted(), "{:?}", analysis.diagnostics);
    assert_eq!(
        analysis.output_attribution,
        NativeRgOutputAttribution::PathOnly
    );
}
