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
                "projectId": "repo-projection",
                "workspaceId": "workspace-projection",
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
