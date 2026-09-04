use std::sync::Mutex;

use agent_semantic_client::LanguageCommandApplication;
use agent_semantic_client::LanguageCommandFuture;
use agent_semantic_client::LanguageCommandOperation;
use agent_semantic_client::LanguageCommandRequest;
use agent_semantic_client_protocol::AspClientSearchRequest;

use super::forward_language_command;
use super::runtime_query_intent;
use super::runtime_search_intent;

#[test]
fn route_intents_are_semantic_and_do_not_forward_argv() {
    let search = runtime_search_intent(
        &[
            "search",
            "playbook",
            "tokio stream",
            "--intent",
            "conceptual",
            "--workspace",
            ".",
        ]
        .map(str::to_owned),
    )
    .expect("search intent");
    assert_eq!(
        search.schema_id,
        "agent.semantic-protocols.asp-client-search-request"
    );
    assert_eq!(search.schema_version, "1");
    assert_eq!(search.query, "tokio stream");
    assert_eq!(search.intent, "conceptual");

    let removed_owner =
        runtime_search_intent(&["search", "owner", "src/lib.rs", "items"].map(str::to_owned))
            .expect_err("owner is not a public Search operation");
    assert!(removed_owner.contains("was removed"));

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
        query.schema_id.as_str(),
        "agent.semantic-protocols.asp-client-exact-query-request"
    );
    assert_eq!(query.selector.as_str(), "src/lib.rs:1:4");
    assert_eq!(query.projection.as_str(), "source");

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
        false
    );
    assert_eq!(
        super::runtime_query_presentation(&["query", "selector", "--json"].map(str::to_owned)),
        true
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

#[derive(Default)]
struct RecordingApplication {
    requests: Mutex<Vec<LanguageCommandRequest>>,
}

impl LanguageCommandApplication for RecordingApplication {
    fn execute(&self, request: LanguageCommandRequest) -> LanguageCommandFuture<'_> {
        self.requests.lock().expect("request lock").push(request);
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn language_cli_boundary_only_forwards_one_typed_request() {
    let application = RecordingApplication::default();
    let project_root = std::path::PathBuf::from("/workspace/project");
    let operation = LanguageCommandOperation::Search(AspClientSearchRequest::playbook(
        "conceptual",
        "typed request",
    ));

    forward_language_command(
        &application,
        "rust",
        operation.clone(),
        project_root.clone(),
        true,
    )
    .await
    .expect("typed forwarding");

    assert_eq!(
        application
            .requests
            .into_inner()
            .expect("recorded requests"),
        vec![LanguageCommandRequest {
            language_id: agent_semantic_client::LanguageId::new("rust"),
            operation,
            project_root,
            machine_readable: true,
        }]
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
