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

fn calibration(command: &str) -> Value {
    let output = evaluate(command).expect("calibration deny");
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
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../schemas/search-playbook-pretool-calibration.v2.schema.json"
    ))
    .expect("calibration schema JSON");
    jsonschema::validator_for(&schema)
        .expect("calibration schema")
        .validate(&calibration)
        .expect("calibration satisfies V2 schema");
    calibration
}

#[test]
fn valid_scheme_composition_passes_without_rewrite_or_calibration() {
    let command = ".devenv/devenv-profile-exec asp search playbook '(search (producers (language rust)) (chain (intersect (rg \"-n\" \"-g\" \"*.rs\" \"-e\" \"Owner|Route\" \"crates\") (tantivy \"title:Owner^2 OR body:Route\")) (syntax rust \"(function_item) @item\") (graph gql \"MATCH (owner:Owner)-[:CALLS]->(target:Item) RETURN owner\")))'";
    assert_eq!(evaluate(command), None);
}

#[test]
fn missing_tantivy_returns_scheme_source_calibration() {
    let calibration = calibration(
        "asp search playbook '(search (producers (language rust)) (intersect (rg \"-n\" \"Owner\" \"crates\")))'",
    );
    assert_eq!(calibration["schemaVersion"], "2");
    assert_eq!(calibration["argumentModel"], "one-scheme-expression");
    assert_eq!(calibration["grammar"]["grammarId"], "tree-sitter-scheme");
    assert_eq!(calibration["grammar"]["topLevelFormHeads"][0], "search");
    assert_eq!(
        calibration["issues"][0]["reasonKind"],
        "search-playbook-request-incomplete"
    );
    assert!(
        calibration["sourceDigest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("blake3-256:"))
    );
}

#[test]
fn malformed_native_rg_returns_the_native_reason() {
    let calibration = calibration(
        "asp search playbook '(search (producers (language rust)) (intersect (rg \"-n\" \"--glob\") (tantivy \"title:Owner^2 OR body:Route\")))'",
    );
    assert_eq!(
        calibration["issues"][0]["reasonKind"],
        "search-playbook-rg-syntax-invalid"
    );
}

#[test]
fn bare_tantivy_text_is_rejected_by_native_analysis() {
    let calibration = calibration(
        "asp search playbook '(search (producers (language rust)) (intersect (rg \"Owner\" \".\") (tantivy \"Owner\")))'",
    );
    assert_eq!(
        calibration["issues"][0]["reasonKind"],
        "search-playbook-tantivy-expression-too-simple"
    );
}

#[test]
fn old_flag_matrix_is_rejected_instead_of_reinterpreted() {
    let calibration = calibration("asp search playbook --language rust --rg Owner .");
    assert_eq!(calibration["expressionArgCount"], 5);
    assert_eq!(
        calibration["issues"][0]["reasonKind"],
        "search-playbook-source-arity-invalid"
    );
}

#[test]
fn missing_expression_has_no_synthetic_source_receipt() {
    let calibration = calibration("asp search playbook");
    assert_eq!(calibration["expressionArgCount"], 0);
    assert!(calibration["sourceDigest"].is_null());
    assert!(calibration["grammar"].is_null());
    assert!(calibration["issues"][0].get("sourceTokenIndex").is_none());
}
