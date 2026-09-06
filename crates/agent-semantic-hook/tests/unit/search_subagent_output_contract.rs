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

fn valid_message() -> String {
    "[asp-search-subagent]\nstate=candidates\nQueryGrammar: asp query --selector <exact-selector> --projection <callable-skeleton|source>\nE1 | owner=crates/runtime | item=struct/Endpoint | selector=rust://crates/runtime#item/struct/Endpoint | matchedBy=rg:0|syntax:0|graph:0 | relation=publishes".to_owned()
}

#[test]
fn admits_ranked_candidate_handoff() {
    assert_eq!(
        evaluate_subagent_stop(&payload(&valid_message())),
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
fn admits_successful_empty_result_without_invented_candidates() {
    assert_eq!(
        evaluate_subagent_stop(&payload("[asp-search-subagent]\nstate=empty")),
        serde_json::json!({})
    );
}

#[test]
fn admits_typed_unavailable_result() {
    assert_eq!(
        evaluate_subagent_stop(&payload(
            "[asp-search-subagent]\nstate=unavailable\nstage=runtime\nreasonKind=transport-unavailable",
        )),
        serde_json::json!({})
    );
}

#[test]
fn blocks_ambiguous_no_output_state() {
    let terminal = evaluate_subagent_stop(&payload("[asp-search-subagent]\nstate=no-output"));
    assert_eq!(terminal["decision"], "block");
}

#[test]
fn blocks_candidates_without_top_k_entries() {
    let terminal = evaluate_subagent_stop(&payload("[asp-search-subagent]\nstate=candidates"));
    assert_eq!(terminal["decision"], "block");
}

#[test]
fn blocks_empty_result_with_candidate_entries() {
    let terminal = evaluate_subagent_stop(&payload(
        "[asp-search-subagent]\nstate=empty\nE1 | owner=crates/runtime | item=struct/Endpoint | selector=rust://crates/runtime#item/struct/Endpoint | matchedBy=syntax:0 | relation=publishes",
    ));
    assert_eq!(terminal["decision"], "block");
}

#[test]
fn blocks_unavailable_result_without_typed_failure() {
    for message in [
        "[asp-search-subagent]\nstate=unavailable",
        "[asp-search-subagent]\nstate=unavailable\nstage=runtime\nreasonKind=",
        "[asp-search-subagent]\nstate=unavailable\nstage=unknown\nreasonKind=transport-unavailable",
    ] {
        let terminal = evaluate_subagent_stop(&payload(message));
        assert_eq!(terminal["decision"], "block");
    }
}

#[test]
fn blocks_source_or_prose_outside_the_closed_grammar() {
    for suffix in [
        "\n```rust\nfn source() {}\n```",
        "\nsource: fn source() {}",
        "\nThis source implements the endpoint.",
    ] {
        let terminal = evaluate_subagent_stop(&payload(&(valid_message() + suffix)));
        assert_eq!(terminal["decision"], "block");
        let context = terminal["reason"].as_str().expect("repair context");
        assert!(context.contains("Example"));
        assert!(context.contains("Grammar"));
    }
}

#[test]
fn blocks_prescribed_next_action() {
    let terminal = evaluate_subagent_stop(&payload(
        &(valid_message() + "\nnext=asp query --selector rust://workspace/item/Endpoint"),
    ));
    assert_eq!(terminal["decision"], "block");
}

#[test]
fn blocks_selector_that_does_not_map_to_the_declared_owner() {
    let terminal = evaluate_subagent_stop(&payload(
        "[asp-search-subagent]\nstate=candidates\nQueryGrammar: asp query --selector <exact-selector> --projection <callable-skeleton|source>\nE1 | owner=crates/runtime | item=struct/Endpoint | selector=rust://crates/client#item/struct/Endpoint | matchedBy=syntax:0 | relation=publishes",
    ));
    assert_eq!(terminal["decision"], "block");
}

#[test]
fn blocks_more_than_top_thirty_evidence_entries() {
    let mut lines = vec![
        "[asp-search-subagent]".to_owned(),
        "state=candidates".to_owned(),
        "QueryGrammar: asp query --selector <exact-selector> --projection <callable-skeleton|source>".to_owned(),
    ];
    lines.extend((1..=31).map(|rank| {
        format!(
            "E{rank} | owner=crates/runtime | item=struct/Item{rank} | selector=rust://crates/runtime#item/struct/Item{rank} | matchedBy=syntax:0 | relation=guards"
        )
    }));
    let message = lines.join("\n");
    let terminal = evaluate_subagent_stop(&payload(&message));
    assert_eq!(terminal["decision"], "block");
}

#[test]
fn blocks_missing_or_inexact_query_grammar() {
    for message in [
        "[asp-search-subagent]\nstate=candidates\nE1 | owner=crates/runtime | item=struct/Endpoint | selector=rust://crates/runtime#item/struct/Endpoint | matchedBy=syntax:0 | relation=publishes",
        "[asp-search-subagent]\nstate=candidates\nQueryGrammar: asp query --selector <exact-selector>\nE1 | owner=crates/runtime | item=struct/Endpoint | selector=rust://crates/runtime#item/struct/Endpoint | matchedBy=syntax:0 | relation=publishes",
    ] {
        assert_eq!(
            evaluate_subagent_stop(&payload(message))["decision"],
            "block"
        );
    }
}

#[test]
fn blocks_item_mismatch_or_invalid_supporting_clause() {
    for message in [
        "[asp-search-subagent]\nstate=candidates\nQueryGrammar: asp query --selector <exact-selector> --projection <callable-skeleton|source>\nE1 | owner=crates/runtime | item=struct/Other | selector=rust://crates/runtime#item/struct/Endpoint | matchedBy=syntax:0 | relation=publishes",
        "[asp-search-subagent]\nstate=candidates\nQueryGrammar: asp query --selector <exact-selector> --projection <callable-skeleton|source>\nE1 | owner=crates/runtime | item=struct/Endpoint | selector=rust://crates/runtime#item/struct/Endpoint | matchedBy=keyword:0 | relation=publishes",
        "[asp-search-subagent]\nstate=candidates\nQueryGrammar: asp query --selector <exact-selector> --projection <callable-skeleton|source>\nE1 | owner=crates/runtime | item=struct/Endpoint | selector=rust://crates/runtime#item/struct/Endpoint | matchedBy=rg:0|rg:0 | relation=publishes",
        "[asp-search-subagent]\nstate=candidates\nQueryGrammar: asp query --selector <exact-selector> --projection <callable-skeleton|source>\nE1 | owner=crates/runtime | item=struct/Endpoint | selector=rust://crates/runtime#item/struct/Endpoint | matchedBy=graph:0|rg:0 | relation=publishes",
    ] {
        assert_eq!(
            evaluate_subagent_stop(&payload(message))["decision"],
            "block"
        );
    }
}

#[test]
fn blocks_missing_explorer_final_message() {
    let terminal = evaluate_subagent_stop(&serde_json::json!({"agent_type": "asp_explorer"}));
    assert_eq!(terminal["decision"], "block");
}

#[test]
fn blocks_missing_agent_identity_to_prevent_contract_bypass() {
    let terminal = evaluate_subagent_stop(&serde_json::json!({}));
    assert_eq!(terminal["decision"], "block");
}

#[test]
fn repair_projection_contains_only_example_and_grammar() {
    let terminal = evaluate_subagent_stop(&payload("source dump"));
    let context = terminal["reason"].as_str().expect("repair contract");
    assert!(context.starts_with("Example\n"));
    assert!(context.contains("\n\nGrammar\n"));
    assert!(!context.contains("rejected"));
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
