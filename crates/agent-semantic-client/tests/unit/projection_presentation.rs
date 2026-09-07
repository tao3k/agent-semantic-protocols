// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client::projection_presentation::{
    ProjectionPresentation, render_exact_projection_response,
    render_workspace_query_playbook_response, render_workspace_search_playbook_gql,
};
use agent_semantic_client_protocol::ClientFrame;
use orgize::Org;
use orgize::ast::ElementData;
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
    response(
        serde_json::from_str(include_str!(
            "../../../../schemas/fixtures/search-topology-settlement/valid-derived-and-proposed.v1.json"
        ))
        .expect("valid Search settlement fixture"),
    )
}

#[test]
fn search_success_is_exactly_one_org_owned_gql_block() {
    let rendered = render_workspace_search_playbook_gql(&search_result()).unwrap();
    let parsed = Org::parse(&rendered);
    let records = parsed.document().source_block_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].language.as_deref(), Some("gql"));
    assert!(rendered.starts_with("#+begin_src gql :profile search-evidence.v1 :eval never\n"));
    assert!(rendered.ends_with("#+end_src\n"));
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
fn query_success_is_gql_keyword_immediately_followed_by_native_source() {
    let frame = response(json!({
        "schemaId": "agent.semantic-protocols.query-playbook-materialization-receipt",
        "schemaVersion": "1",
        "materializations": [{
            "selector": "rust://src/registry.rs#item/function/refresh_registry",
            "languageId": "rust",
            "providerId": "asp-rust",
            "ownerPath": "src/registry.rs",
            "projection": "source",
            "sourceContentDigest": "b".repeat(64),
            "gqlRelationships": [{
                "fromNode": "Registry::refresh",
                "relation": "calls",
                "toNode": "Registry::publish"
            }],
            "bytes": [112, 117, 98, 32, 102, 110, 32, 114, 101, 102, 114, 101, 115, 104, 95, 114, 101, 103, 105, 115, 116, 114, 121, 40, 41, 32, 123, 125]
        }],
        "terminal": {"state": "ready", "terminalCount": 1}
    }));
    let rendered =
        render_workspace_query_playbook_response(&frame, ProjectionPresentation::Text).unwrap();
    assert_eq!(
        rendered,
        "#+GQL: Registry::refresh --calls--> Registry::publish\n#+begin_src rust :query \"rust://src/registry.rs#item/function/refresh_registry\" :filename \"src/registry.rs\"\npub fn refresh_registry() {}\n#+end_src\n"
    );
    let parsed = Org::parse(&rendered);
    assert_eq!(parsed.document().children.len(), 2);
    assert!(matches!(
        &parsed.document().children[0].data,
        ElementData::Keyword(keyword) if keyword.key.eq_ignore_ascii_case("gql")
    ));
    assert_eq!(parsed.document().source_block_records().len(), 1);
    assert!(!rendered.contains("gqlAffiliation"));
}

#[test]
fn query_rejects_missing_relationship_instead_of_exposing_bare_source() {
    let frame = response(json!({
        "schemaId": "agent.semantic-protocols.query-playbook-materialization-receipt",
        "schemaVersion": "1",
        "materializations": [{
            "selector": "rust://src/lib.rs#item/function/run",
            "languageId": "rust",
            "providerId": "asp-rust",
            "ownerPath": "src/lib.rs",
            "projection": "source",
            "sourceContentDigest": "b".repeat(64),
            "gqlRelationships": [],
            "bytes": [102, 110, 32, 114, 117, 110, 40, 41, 32, 123, 125]
        }],
        "terminal": {"state": "ready", "terminalCount": 1}
    }));
    assert!(
        render_workspace_query_playbook_response(&frame, ProjectionPresentation::Text)
            .unwrap_err()
            .contains("exactly one GQL relationship")
    );
}

#[test]
fn machine_presentation_preserves_the_typed_query_receipt() {
    let frame = response(json!({"result": {"bytes": [102, 110, 32, 109, 97, 105, 110]}}));
    let machine =
        render_exact_projection_response(&frame, ProjectionPresentation::MachineJson).unwrap();
    assert!(machine.contains("\"bytes\":[102,110,32,109,97,105,110"));
}
