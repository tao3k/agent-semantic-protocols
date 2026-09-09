// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client::projection_presentation::{
    ProjectionPresentation, render_exact_projection_response,
    render_workspace_query_playbook_response, render_workspace_search_playbook_gql,
};
use agent_semantic_client_protocol::ClientFrame;
use orgize::Org;
use serde_json::{Value, json};

fn response(result: Value) -> ClientFrame {
    serde_json::from_value(json!({
        "kind": "response",
        "schemaId": "agent.semantic-protocols.client.frame",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "sessionId": "session-projection",
        "projectId": "repo-projection",
        "workspaceId": "workspace-projection",
        "requestId": "request-projection",
        "outcome": "ready",
        "result": result,
        "error": null,
        "catalog": null
    }))
    .expect("typed response fixture")
}

#[test]
fn renders_provider_projection_bytes_as_utf8_text() {
    let frame = response(json!({
        "result": {"bytes": [112, 117, 98, 32, 102, 110, 32, 114, 101, 97, 100, 121, 40, 41, 32, 123, 125]}
    }));
    assert_eq!(
        render_exact_projection_response(&frame, ProjectionPresentation::Text).unwrap(),
        "pub fn ready() {}"
    );
}

#[test]
fn rejects_non_byte_projection_values() {
    let frame = response(json!({"result": {"bytes": [256]}}));
    assert_eq!(
        render_exact_projection_response(&frame, ProjectionPresentation::Text).unwrap_err(),
        "exact projection response contains a non-byte value"
    );
}

#[test]
fn does_not_expand_an_owner_repair_packet_as_an_exact_projection() {
    let frame = response(json!({
        "result": {
            "state": "owner-for-repair",
            "owner": {"bytes": [101, 110, 116, 105, 114, 101, 32, 111, 119, 110, 101, 114]}
        }
    }));
    assert_eq!(
        render_exact_projection_response(&frame, ProjectionPresentation::Text).unwrap_err(),
        "exact projection response has no text or byte payload"
    );
}

#[test]
fn preserves_runtime_exact_query_failure_terminal() {
    let frame: ClientFrame = serde_json::from_value(json!({
        "kind": "response",
        "schemaId": "agent.semantic-protocols.client.frame",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "sessionId": "session-projection",
        "projectId": "repo-projection",
        "workspaceId": "workspace-projection",
        "requestId": "request-projection",
        "outcome": "error",
        "result": null,
        "error": {
            "reasonKind": "projection-missing",
            "message": "exact projection terminal: projection-missing",
            "terminal": {
                "schemaId": "agent.semantic-protocols.asp-client-exact-query-failure",
                "schemaVersion": "1",
                "state": "failed",
                "operationId": "request-projection",
                "projectId": "repo-projection",
                "workspaceId": "workspace-projection",
                "languageId": "rust",
                "providerId": "asp-rust",
                "requestedSelector": "rust://src/lib.rs#item/function/missing",
                "resolvedSelector": "rust://src/lib.rs#item/function/missing",
                "projectionKind": "source",
                "phase": "resident-selector-read",
                "reasonKind": "projection-missing",
                "generationDigest": format!("blake3-256:{}", "a".repeat(64)),
                "rootDigest": "b".repeat(64),
                "residentReadElapsedMicros": 7,
                "serviceElapsedMicros": 3,
                "elapsedMicros": 10,
                "workCounters": {
                    "databaseReadCount": 0,
                    "filesystemReadCount": 0,
                    "providerProcessCount": 0,
                    "schedulerTaskCount": 0,
                    "socketOperationCount": 0
                },
                "details": {"selectorState": "projection-missing"}
            }
        },
        "catalog": null
    }))
    .expect("typed failure frame");

    assert_eq!(
        render_exact_projection_response(&frame, ProjectionPresentation::Text).unwrap_err(),
        "exact projection terminal: projection-missing"
    );
    let machine = render_exact_projection_response(&frame, ProjectionPresentation::MachineJson)
        .expect("explicit JSON preserves the typed terminal");
    assert!(machine.contains("\"reasonKind\":\"projection-missing\""));
    assert!(machine.contains("\"phase\":\"resident-selector-read\""));
    assert!(!machine.contains("\"recommendedNext\""));
}

fn search_result() -> ClientFrame {
    let mut packet: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/search-topology-settlement/valid-derived-and-proposed.v1.json"
    ))
    .expect("valid Search settlement fixture");
    packet["schemaVersion"] = serde_json::json!("2");
    packet["resultState"] = serde_json::json!("queryable");
    packet.as_object_mut().unwrap().remove("materializationSet");
    response(packet)
}

#[test]
fn search_success_is_exactly_one_org_owned_gql_block() {
    let rendered = render_workspace_search_playbook_gql(&search_result()).unwrap();
    let parsed = Org::parse(&rendered);
    let records = parsed.document().source_block_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].language.as_deref(), Some("gql"));
    assert!(rendered.starts_with("#+begin_src gql :name result\n"));
    assert!(rendered.ends_with("#+end_src\n"));
    assert!(rendered.contains("(rust:Language)-[:RESULTS]->["));
    assert!(rendered.contains(
        "projection:{rank:1,depth:0,hit:{rg:[[42,46]],tantivy:[\"artifact refresh\"],native:true}}"
    ));
    assert!(!rendered.contains("MaterializationSet"));
    for forbidden in [
        "[search-result]",
        "QueryGrammar",
        "Example",
        "Grammar",
        "recommendedNext",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "forbidden legacy output: {forbidden}"
        );
    }
}

#[test]
fn search_rejects_internal_plan_and_invalid_settlement() {
    let plan = response(json!({
        "schemaId": "agent.semantic-protocols.workspace-search-playbook-plan",
        "schemaVersion": "1",
        "result": "plan-ready",
        "evidence": []
    }));
    assert!(
        render_workspace_search_playbook_gql(&plan)
            .unwrap_err()
            .contains("schema")
    );

    let mut invalid: Value = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/search-topology-settlement/valid-derived-and-proposed.v1.json"
    ))
    .unwrap();
    invalid["rendering"]["gqlBlockCount"] = json!(2);
    assert!(
        render_workspace_search_playbook_gql(&response(invalid))
            .unwrap_err()
            .contains("exactly one GQL block")
    );
}

#[test]
fn query_success_is_only_native_result_blocks_in_request_order() {
    let rust_selector = "rust://src/registry.rs#item/function/refresh_registry";
    let json_selector = "json://schemas/project-config.schema.json#item/pointer/properties/hook";
    let frame = response(json!({
        "schemaId": "agent.semantic-protocols.query-playbook-materialization-receipt",
        "schemaVersion": "1",
        "requestedSelectors": [rust_selector, json_selector],
        "materializations": [
        {
            "selector": rust_selector,
            "languageId": "rust",
            "providerId": "asp-rust",
            "ownerPath": "src/registry.rs",
            "projection": "source",
            "sourceContentDigest": "b".repeat(64),
            "bytes": [112, 117, 98, 32, 102, 110, 32, 114, 101, 102, 114, 101, 115, 104, 95, 114, 101, 103, 105, 115, 116, 114, 121, 40, 41, 32, 123, 125]
        },
        {
            "selector": json_selector,
            "languageId": "json",
            "providerId": "asp-json",
            "ownerPath": "schemas/project-config.schema.json",
            "projection": "source",
            "sourceContentDigest": "c".repeat(64),
            "bytes": [123, 34, 101, 110, 97, 98, 108, 101, 100, 34, 58, 116, 114, 117, 101, 125]
        }],
        "terminal": {"state": "ready", "terminalCount": 1}
    }));
    let rendered =
        render_workspace_query_playbook_response(&frame, ProjectionPresentation::Text).unwrap();
    assert_eq!(
        rendered,
        "#+begin_src rust :query \"rust://src/registry.rs#item/function/refresh_registry\" :filename \"src/registry.rs\"\npub fn refresh_registry() {}\n#+end_src\n\n#+begin_src json :query \"json://schemas/project-config.schema.json#item/pointer/properties/hook\" :filename \"schemas/project-config.schema.json\"\n{\"enabled\":true}\n#+end_src\n"
    );
    let parsed = Org::parse(&rendered);
    assert_eq!(parsed.document().children.len(), 2);
    assert_eq!(parsed.document().source_block_records().len(), 2);
    assert!(!rendered.to_ascii_lowercase().contains("gql"));
}

#[test]
fn query_failure_and_order_drift_cannot_be_rendered_as_successful_concat() {
    let frame = response(json!({
        "schemaId": "agent.semantic-protocols.query-playbook-materialization-receipt",
        "schemaVersion": "1",
        "requestedSelectors": ["rust://src/lib.rs#item/function/run"],
        "materializations": [],
        "terminal": {
            "state": "failed",
            "terminalCount": 1,
            "reasonKind": "query-playbook-selector-not-materialized"
        }
    }));
    assert!(
        render_workspace_query_playbook_response(&frame, ProjectionPresentation::Text)
            .unwrap_err()
            .contains("reasonKind=query-playbook-selector-not-materialized")
    );

    let mut drifted = response(json!({
        "schemaId": "agent.semantic-protocols.query-playbook-materialization-receipt",
        "schemaVersion": "1",
        "requestedSelectors": [
            "rust://src/a.rs#item/function/a",
            "rust://src/b.rs#item/function/b"
        ],
        "materializations": [],
        "terminal": {"state": "ready", "terminalCount": 1}
    }));
    if let ClientFrame::Response {
        result: Some(receipt),
        ..
    } = &mut drifted
    {
        receipt["materializations"] = json!([{
            "selector": "rust://src/b.rs#item/function/b",
            "languageId": "rust",
            "providerId": "asp-rust",
            "ownerPath": "src/b.rs",
            "projection": "source",
            "sourceContentDigest": "b".repeat(64),
            "bytes": [98]
        }, {
            "selector": "rust://src/a.rs#item/function/a",
            "languageId": "rust",
            "providerId": "asp-rust",
            "ownerPath": "src/a.rs",
            "projection": "source",
            "sourceContentDigest": "a".repeat(64),
            "bytes": [97]
        }]);
    }
    assert!(
        render_workspace_query_playbook_response(&drifted, ProjectionPresentation::Text)
            .unwrap_err()
            .contains("do not preserve the complete request order")
    );
}

#[test]
fn machine_presentation_preserves_the_typed_query_receipt() {
    let frame = response(json!({"result": {"bytes": [102, 110, 32, 109, 97, 105, 110]}}));
    let machine =
        render_exact_projection_response(&frame, ProjectionPresentation::MachineJson).unwrap();
    assert!(machine.contains("\"bytes\":[102,110,32,109,97,105,110"));
}
