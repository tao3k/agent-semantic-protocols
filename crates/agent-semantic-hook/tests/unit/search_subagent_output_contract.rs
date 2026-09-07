// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::evaluate_subagent_stop;

const CODEX_SUBAGENT_STOP_OUTPUT_SCHEMA: &str = include_str!(
    "../../../agent-semantic-config/schemas/codex-hooks/subagent-stop-command-output.schema.json"
);

fn payload(message: &str) -> serde_json::Value {
    serde_json::json!({
        "agent_type": "asp_explorer",
        "last_assistant_message": message,
    })
}

fn valid_message() -> &'static str {
    "#+begin_src gql :profile search-evidence.v1 :eval never\n(search:SearchResult {state:\"materializable\"})\n(search)-[:RESULTS]->(item:RustFunction {selector:\"rust://crates/runtime#item/function/serve\"})\n#+end_src\n"
}

#[test]
fn admits_exactly_one_org_owned_gql_handoff() {
    assert_eq!(
        evaluate_subagent_stop(&payload(valid_message())),
        serde_json::json!({})
    );
}

#[test]
fn admits_an_empty_search_as_a_gql_graph_not_a_flat_state() {
    assert_eq!(
        evaluate_subagent_stop(&payload(
            "#+begin_src gql :profile search-evidence.v1 :eval never\n(search:SearchResult {state:\"empty\"})\n#+end_src\n"
        )),
        serde_json::json!({})
    );
}

#[test]
fn leaves_other_agent_terminals_observational() {
    assert_eq!(
        evaluate_subagent_stop(&serde_json::json!({
            "agent_type": "asp_testing",
            "last_assistant_message": "arbitrary test receipt"
        })),
        serde_json::json!({})
    );
}

#[test]
fn blocks_legacy_and_non_org_search_formats() {
    for message in [
        "[asp-search-subagent]\nstate=empty",
        "[search-result] result=no-match evidence=0",
        "QueryGrammar: asp query --selector <exact-selector>",
        "```gql\n(search:SearchResult)\n```",
        "prose before\n#+begin_src gql :profile search-evidence.v1 :eval never\n(search:SearchResult)\n#+end_src\n",
    ] {
        assert_eq!(
            evaluate_subagent_stop(&payload(message))["decision"],
            "block"
        );
    }
}

#[test]
fn blocks_wrong_or_missing_gql_headers_and_multiple_blocks() {
    for message in [
        "#+begin_src gql\n(search:SearchResult)\n#+end_src\n",
        "#+begin_src gql :profile search-evidence.v1 :eval yes\n(search:SearchResult)\n#+end_src\n",
        "#+begin_src rust :profile search-evidence.v1 :eval never\nfn search() {}\n#+end_src\n",
        "#+begin_src gql :profile search-evidence.v1 :eval never\n(search:SearchResult)\n#+end_src\n#+begin_src gql :profile search-evidence.v1 :eval never\n(other:SearchResult)\n#+end_src\n",
    ] {
        assert_eq!(
            evaluate_subagent_stop(&payload(message))["decision"],
            "block"
        );
    }
}

#[test]
fn blocks_missing_identity_or_final_message() {
    assert_eq!(
        evaluate_subagent_stop(&serde_json::json!({}))["decision"],
        "block"
    );
    assert_eq!(
        evaluate_subagent_stop(&serde_json::json!({"agent_type": "asp_explorer"}))["decision"],
        "block"
    );
}

#[test]
fn repair_projection_is_concise_and_contains_no_legacy_example_or_grammar() {
    let terminal = evaluate_subagent_stop(&payload("source dump"));
    let reason = terminal["reason"].as_str().expect("repair guidance");
    assert!(reason.contains("exactly one Org source block"));
    for forbidden in [
        "Example",
        "Grammar",
        "QueryGrammar",
        "[asp-search-subagent]",
    ] {
        assert!(
            !reason.contains(forbidden),
            "legacy guidance leaked: {forbidden}"
        );
    }
    assert_eq!(terminal.as_object().expect("Host object").len(), 2);
    let schema: serde_json::Value =
        serde_json::from_str(CODEX_SUBAGENT_STOP_OUTPUT_SCHEMA).expect("Host output schema");
    assert!(
        jsonschema::validator_for(&schema)
            .expect("compile Host output schema")
            .is_valid(&terminal),
        "terminal={terminal}"
    );
}
