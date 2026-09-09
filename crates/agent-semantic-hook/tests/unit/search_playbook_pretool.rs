// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use serde_json::Value;

fn evaluate(command: &str) -> Option<Value> {
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": command}
    })
    .to_string();
    agent_semantic_hook::evaluate_search_playbook_pretool(&payload, "Bash")
        .expect("evaluate calibration")
}

#[test]
fn valid_complex_rg_argv_passes_without_rewrite_or_calibration() {
    let command = ".devenv/devenv-profile-exec asp search playbook --language rust --rg -n -g '*.rs' --type rust -e 'Owner|Route' crates --tantivy 'title:Owner^2 OR Route' --graph gql 'MATCH (a)-[:CALLS]->(b) RETURN a,b'";
    assert_eq!(evaluate(command), None);
}

#[test]
fn missing_tantivy_returns_one_typed_fill_form_calibration() {
    let output =
        evaluate("asp search playbook --language rust --rg -n -g '*.rs' --type rust Owner crates")
            .expect("calibration deny");
    assert_eq!(
        output
            .pointer("/hookSpecificOutput/permissionDecision")
            .and_then(Value::as_str),
        Some("deny")
    );
    let context = output
        .pointer("/hookSpecificOutput/additionalContext")
        .and_then(Value::as_str)
        .expect("typed additional context");
    let calibration: Value = serde_json::from_str(context).expect("calibration JSON");
    assert_eq!(
        calibration.get("reasonKind").and_then(Value::as_str),
        Some("search-playbook-pretool-calibration")
    );
    assert_eq!(calibration["missingFields"], serde_json::json!(["tantivy"]));
    assert_eq!(
        calibration["rgArgvBoundaries"][0]["argv"],
        serde_json::json!(["-n", "-g", "*.rs", "--type", "rust", "Owner", "crates"])
    );
    assert_eq!(calibration["rgAnalyses"][0]["admitted"], true);
    assert_eq!(
        calibration["rgAnalyses"][0]["outputAttribution"],
        "path-line"
    );
    assert!(!context.contains("guide"));
}

#[test]
fn malformed_native_rg_returns_option_level_fill_form_evidence() {
    let output = evaluate("asp search playbook --language rust --rg -n --glob")
        .expect("native rg calibration deny");
    let context = output["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("context");
    let calibration: Value = serde_json::from_str(context).expect("calibration JSON");
    assert_eq!(calibration["rgAnalyses"][0]["admitted"], false);
    assert!(
        calibration["rgAnalyses"][0]["diagnostics"]
            .as_array()
            .expect("rg diagnostics")
            .iter()
            .any(|diagnostic| {
                diagnostic["reasonKind"] == "search-playbook-rg-option-value-missing"
            })
    );
}

#[test]
fn bare_tantivy_text_is_rejected_with_native_ast_metrics() {
    let output = evaluate("asp search playbook --language rust --rg Owner . --tantivy Owner")
        .expect("simple Tantivy calibration");
    let context = output["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("context");
    let calibration: Value = serde_json::from_str(context).expect("calibration JSON");
    assert_eq!(calibration["tantivyAnalyses"][0]["admitted"], false);
    assert_eq!(calibration["tantivyAnalyses"][0]["metrics"]["leafCount"], 1);
    assert!(
        calibration["tantivyAnalyses"][0]["missingFeatures"]
            .as_array()
            .expect("missing features")
            .contains(&Value::String("explicit-boolean-composition".to_owned()))
    );
}
