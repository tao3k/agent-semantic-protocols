use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use agent_semantic_client_protocol::{
    CLIENT_FRAME_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION, ClientFrame,
    ClientFrameBase, ClientInfo, ClientRequestId, ClientSessionId, ClientWorkspaceIdentity,
    SCHEMA_VERSION,
};
use agent_semantic_http_json::HttpJsonRequest;
use agent_semantic_schema_manager::SchemaManager;

fn digest(character: char) -> String {
    format!("blake3-256:{}", character.to_string().repeat(64))
}

fn registered_language_provider_pairs() -> Vec<(String, String)> {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    SchemaManager::new(workspace_root)
        .registered_language_profiles()
        .expect("registered language profiles")
        .into_iter()
        .map(|profile| {
            let provider_id = format!("asp-{}", profile.language_id);
            (profile.language_id, provider_id)
        })
        .collect()
}

async fn dispatch_without_initialize_admits_the_pinned_provider_candidate(
    request_id: &str,
    method: &str,
    params: serde_json::Value,
    language_id: &str,
    provider_id: &str,
) {
    let directory = tempfile::tempdir().expect("temporary workspace");
    let project_root = directory.path().join("project");
    std::fs::create_dir_all(&project_root).expect("create project root");
    std::fs::write(project_root.join("lib.rs"), "pub fn ready() {}\n")
        .expect("write source fixture");
    let workspace_identity =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
            .expect("workspace identity");
    let builds = Arc::new(AtomicUsize::new(0));
    let observed = Arc::new(std::sync::Mutex::new(None));
    let admission = Arc::new(
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission::new(
            Arc::new({
                let builds = Arc::clone(&builds);
                let observed = Arc::clone(&observed);
                move |_, _, candidate, _, _, provider_target, _| {
                    builds.fetch_add(1, Ordering::AcqRel);
                    *observed.lock().expect("generation observation lock") =
                        Some((candidate.candidate_generation.digest, provider_target));
                    Box::pin(async {
                        Err(agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationFailureStage::GenerationBuilder,
                            "stop after observing pinned HTTP demand",
                        ))
                    })
                }
            }),
        ),
    );
    let registry = Arc::new(
        agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
            directory.path().join("runtime"),
        )
        .expect("workspace registry"),
    );
    let (runtime_search_service, _runtime_search_requests) =
        agent_semantic_client_db::runtime_search_service::runtime_search_service_channel();
    let telemetry = agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus::new();
    let service = agent_semantic_runtime_server::build_http_service(
        runtime_search_service,
        registry,
        digest('a'),
        Arc::from(registered_language_provider_pairs()),
        admission,
        agent_semantic_runtime_server::query_generation::RuntimeQueryGenerationAuthority::new(),
        telemetry.sender,
    )
    .expect("HTTP service");
    let base = ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: ClientSessionId::new("session-initialize").expect("session id"),
        workspace_identity: ClientWorkspaceIdentity::new(workspace_identity)
            .expect("workspace identity"),
        trace_context: None,
    };
    let request = ClientFrame::Dispatch {
        base,
        request_id: ClientRequestId::new(request_id).expect("request id"),
        project_root: project_root.to_string_lossy().into_owned(),
        client_info: ClientInfo {
            name: "runtime-test".to_owned(),
            version: "1".to_owned(),
        },
        method: method.to_owned(),
        params,
    };
    let response = service
        .handle(HttpJsonRequest {
            method: "POST".to_owned(),
            path: "/protocol/frame".to_owned(),
            body: serde_json::to_vec(&request)
                .expect("encode dispatch")
                .into(),
        })
        .await
        .expect("dispatch response");

    assert_eq!(response.status, 200);
    let response_frame: ClientFrame =
        serde_json::from_slice(&response.body).expect("decode dispatch response");

    assert!(
        matches!(response_frame, ClientFrame::Response { error: Some(_), .. }),
        "generation failure must be a typed client response: {response_frame:?}"
    );
    assert_eq!(
        builds.load(Ordering::Acquire),
        1,
        "dispatch response: {}",
        String::from_utf8_lossy(&response.body)
    );
    let (candidate_digest, provider_target) = observed
        .lock()
        .expect("generation observation lock")
        .clone()
        .expect("dispatch generation observation");
    let normalized_candidate_digest = candidate_digest.replacen("blake3:", "blake3-256:", 1);
    assert!(normalized_candidate_digest.starts_with("blake3-256:"));
    assert_eq!(normalized_candidate_digest.len(), "blake3-256:".len() + 64);
    assert_eq!(
        provider_target,
        Some(
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget {
                language_id: language_id.to_owned(),
                provider_id: Some(provider_id.to_owned()),
            }
        )
    );
}

#[tokio::test]
async fn search_dispatch_admits_the_pinned_provider_candidate() {
    dispatch_without_initialize_admits_the_pinned_provider_candidate(
        "request-search",
        "rust.search",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.asp-client-search-request",
            "schemaVersion": "1",
            "operation": "pipe"
        }),
        "rust",
        "asp-rust",
    )
    .await;
}

#[tokio::test]
async fn exact_query_dispatch_admits_the_pinned_provider_candidate() {
    dispatch_without_initialize_admits_the_pinned_provider_candidate(
        "request-query",
        "rust.query",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.asp-client-exact-query-request",
            "schemaVersion": "1",
            "selector": "rust://src/lib.rs#item/function/ready",
            "projection": "source"
        }),
        "rust",
        "asp-rust",
    )
    .await;
}

#[tokio::test]
async fn registered_language_search_routes_share_runtime_admission() {
    for (language_id, provider_id) in registered_language_provider_pairs() {
        dispatch_without_initialize_admits_the_pinned_provider_candidate(
            &format!("request-{language_id}-search"),
            &format!("{language_id}.search"),
            serde_json::json!({
                "schemaId": "agent.semantic-protocols.asp-client-search-request",
                "schemaVersion": "1",
                "operation": "pipe"
            }),
            &language_id,
            &provider_id,
        )
        .await;
    }
}
