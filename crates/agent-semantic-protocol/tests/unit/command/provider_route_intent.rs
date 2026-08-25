use super::{runtime_owner_intent, runtime_query_intent, runtime_search_intent};

#[test]
fn route_intents_are_semantic_and_do_not_forward_argv() {
    let search = runtime_search_intent(
        &["search", "pipe", "tokio stream", "--workspace", "."].map(str::to_owned),
    )
    .expect("search intent");
    assert_eq!(
        search["schemaId"],
        "agent.semantic-protocols.asp-client-search-request"
    );
    assert_eq!(search["schemaVersion"], "1");
    assert_eq!(search["query"], "tokio stream");
    assert_eq!(search["operation"], "pipe");
    assert!(search.get("argv").is_none());

    let owner = runtime_owner_intent(
        &[
            "search",
            "owner",
            "src/lib.rs",
            "items",
            "--query",
            "Runtime",
        ]
        .map(str::to_owned),
    )
    .expect("owner intent");
    assert_eq!(
        owner["schemaId"],
        "agent.semantic-protocols.asp-client-owner-search-request"
    );
    assert_eq!(owner["ownerPath"], "src/lib.rs");
    assert_eq!(owner["query"], "Runtime");
    assert!(owner.get("argv").is_none());

    let query = runtime_query_intent(
        &[
            "query",
            "--selector",
            "src/lib.rs:1:4",
            "--projection",
            "source",
        ]
        .map(str::to_owned),
    )
    .expect("query intent");
    assert_eq!(
        query["schemaId"],
        "agent.semantic-protocols.asp-client-exact-query-request"
    );
    assert_eq!(query["selector"], "src/lib.rs:1:4");
    assert_eq!(query["projection"], "source");
    assert!(query.get("argv").is_none());

    let machine_query = runtime_query_intent(
        &[
            "query",
            "src/lib.rs:1:4",
            "--projection",
            "source",
            "--json",
        ]
        .map(str::to_owned),
    )
    .expect("machine query intent");
    assert_eq!(
        machine_query, query,
        "presentation mode is client-owned and must not alter the Runtime request schema"
    );
    assert_eq!(
        super::runtime_query_presentation(&["query", "selector"].map(str::to_owned)),
        super::ProjectionPresentation::Text
    );
    assert_eq!(
        super::runtime_query_presentation(&["query", "selector", "--json"].map(str::to_owned)),
        super::ProjectionPresentation::MachineJson
    );

    let positional_query = runtime_query_intent(
        &["query", "src/lib.rs:1:4", "--projection", "source"].map(str::to_owned),
    )
    .expect("positional query intent");
    assert_eq!(
        positional_query, query,
        "accepted exact-query command shapes must normalize to one Runtime intent"
    );
}

#[test]
fn registered_language_schema_profiles_share_projection_presentation() {
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace root");
    let profiles = agent_semantic_schema_manager::SchemaManager::new(&workspace_root)
        .registered_language_profiles()
        .expect("registered language schema profiles");
    assert!(!profiles.is_empty(), "registered language schema is empty");

    for profile in profiles {
        let frame: agent_semantic_client_protocol::ClientFrame =
            serde_json::from_value(serde_json::json!({
                "kind": "response",
                "schemaId": "agent.semantic-protocols.client.frame",
                "schemaVersion": "1",
                "protocolId": "agent.semantic-protocols.client",
                "protocolVersion": "1",
                "sessionId": "session-projection",
                "workspaceIdentity": "workspace-projection",
                "requestId": "request-projection",
                "outcome": "ready",
                "result": {
                    "languageId": profile.language_id.clone(),
                    "result": {"bytes": [115, 111, 117, 114, 99, 101]}
                },
                "error": null,
                "catalog": null
            }))
            .expect("typed projection response");
        assert_eq!(
            agent_semantic_client::projection_presentation::render_exact_projection_response(
                &frame,
                agent_semantic_client::projection_presentation::ProjectionPresentation::Text,
            )
            .expect("text projection"),
            "source",
            "languageId={}",
            profile.language_id
        );
    }
}
