use serde_json::json;

#[test]
fn northbound_client_requests_are_distinct_from_provider_runtime_requests() {
    let search = crate::AspClientSearchRequest {
        schema_id: "agent.semantic-protocols.asp-client-search-request".to_owned(),
        schema_version: "1".to_owned(),
        operation: "pipe".to_owned(),
        query: "RuntimeAspClient".to_owned(),
    };
    search.validate_schema_identity().expect("search identity");

    let exact = crate::AspClientExactQueryRequest {
        schema_id: "agent.semantic-protocols.asp-client-exact-query-request".to_owned(),
        schema_version: "1".to_owned(),
        selector: "rust://src/lib.rs#item/function/example".to_owned(),
        projection: "callable-skeleton".to_owned(),
    };
    exact.validate_schema_identity().expect("query identity");

    let owner = crate::AspClientOwnerSearchRequest {
        schema_id: "agent.semantic-protocols.asp-client-owner-search-request".to_owned(),
        schema_version: "1".to_owned(),
        owner_path: "src/lib.rs".to_owned(),
        query: "example".to_owned(),
        view: "seeds".to_owned(),
    };
    owner.validate_schema_identity().expect("owner identity");

    assert_ne!(
        search.schema_id,
        "agent.semantic-protocols.runtime-provider-search-request"
    );
}

#[test]
fn provider_route_bindings_roundtrip_and_reject_identity_drift() {
    let request = crate::RuntimeProviderSearchRequest {
        schema_id: "agent.semantic-protocols.runtime-provider-search-request".to_owned(),
        schema_version: "1".to_owned(),
        operation_id: "op".to_owned(),
        workspace_identity: "workspace".to_owned(),
        language_id: "rust".to_owned(),
        scope: "production".to_owned(),
        query_plan: json!({"method":"lexical","terms":["owner"],"view":"seeds"}),
    };
    let encoded = serde_json::to_vec(&request).expect("encode");
    let decoded: crate::RuntimeProviderSearchRequest =
        serde_json::from_slice(&encoded).expect("decode");
    assert!(decoded.validate_schema_identity().is_ok());
    let mut invalid = decoded;
    invalid.schema_id.push_str(".v1");
    assert!(invalid.validate_schema_identity().is_err());
}

use super::*;

fn digest(character: char) -> String {
    format!("blake3-256:{}", character.to_string().repeat(64))
}

fn request_id(value: &str) -> ClientRequestId {
    ClientRequestId::new(value).expect("request id")
}

fn catalog() -> ClientProtocolCatalog {
    ClientProtocolCatalog {
        schema_id: CLIENT_CATALOG_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        catalog_generation: digest('a'),
        workspace_generation: digest('b'),
        transports: vec![ClientTransport::HttpJson, ClientTransport::RuntimeIpc],
        capabilities: ClientCapabilities {
            request_cancellation: true,
            events: true,
            streaming: false,
            trace_context: true,
        },
        methods: vec![ClientMethod {
            method: "rust.search".to_owned(),
            route_id: "rust.search".to_owned(),
            request_schema_id: "request-schema".to_owned(),
            response_schema_id: "response-schema".to_owned(),
            error_schema_ids: vec!["error-schema".to_owned()],
            parameters: vec![ClientParameter {
                name: "query".to_owned(),
                value_type: ClientParameterType::String,
                cardinality: ClientParameterCardinality::Required,
                source: ClientParameterSource::Request,
            }],
            cancellable: true,
            streaming: false,
        }],
    }
}

fn base() -> ClientFrameBase {
    ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: ClientSessionId::new("session").expect("session id"),
        workspace_identity: ClientWorkspaceIdentity::new("workspace").expect("workspace id"),
        trace_context: None,
    }
}

#[test]
fn catalog_is_closed_and_generation_pinned() {
    let catalog = catalog();
    catalog.validate().expect("valid catalog");
    let unknown = ClientFrame::Request {
        base: base(),
        request_id: request_id("request"),
        catalog_generation: catalog.catalog_generation.clone(),
        workspace_generation: catalog.workspace_generation.clone(),
        method: "python.query".to_owned(),
        params: json!({}),
    };
    let error = ClientSessionState::Initialized
        .admit(&unknown, &catalog)
        .expect_err("unknown method must fail");
    assert_eq!(error.reason_kind, "method-not-in-client-catalog");
}

#[test]
fn client_lifecycle_has_no_provider_control_transition() {
    let catalog = catalog();
    let initialize = ClientFrame::Initialize {
        base: base(),
        request_id: request_id("initialize"),
        project_root: "/workspace".to_owned(),
        client_info: ClientInfo {
            name: "test".to_owned(),
            version: "1".to_owned(),
        },
        capabilities: json!({}),
    };
    let state = ClientSessionState::Created
        .admit(&initialize, &catalog)
        .expect("initialize");
    let shutdown = ClientFrame::Shutdown {
        base: base(),
        request_id: request_id("shutdown"),
    };
    let state = state.admit(&shutdown, &catalog).expect("shutdown");
    let exit = ClientFrame::Exit { base: base() };
    assert_eq!(
        state.admit(&exit, &catalog).expect("exit"),
        ClientSessionState::Exited
    );
}

#[test]
fn cancellation_preserves_initialized_session() {
    let catalog = catalog();
    let cancel = ClientFrame::Cancel {
        base: base(),
        request_id: request_id("request"),
    };
    assert_eq!(
        ClientSessionState::Initialized
            .admit(&cancel, &catalog)
            .expect("cancel"),
        ClientSessionState::Initialized
    );
}

#[test]
fn request_parameters_are_executable_and_fail_closed() {
    let catalog = catalog();
    let request = ClientFrame::Request {
        base: base(),
        request_id: request_id("request"),
        catalog_generation: catalog.catalog_generation.clone(),
        workspace_generation: catalog.workspace_generation.clone(),
        method: "rust.search".to_owned(),
        params: json!({"query": 42}),
    };
    let error = ClientSessionState::Initialized
        .admit(&request, &catalog)
        .expect_err("wrong parameter type must fail");
    assert_eq!(error.reason_kind, "client-request-parameter-type-mismatch");
}

#[test]
fn catalog_parameter_identity_matches_provider_route_input_slots() {
    let mut catalog = catalog();
    catalog.methods[0].parameters[0].name = "ownerPath".to_owned();
    catalog.validate().expect("camelCase provider input slot");

    catalog.methods[0].parameters[0].name = "owner-path".to_owned();
    let error = catalog
        .validate()
        .expect_err("kebab name must drift closed");
    assert_eq!(error.reason_kind, "client-parameter-name-invalid");
}

#[test]
fn runtime_context_cannot_be_injected_by_a_client() {
    let mut catalog = catalog();
    catalog.methods[0].parameters.push(ClientParameter {
        name: "workspace".to_owned(),
        value_type: ClientParameterType::WorkspaceRelativePath,
        cardinality: ClientParameterCardinality::Required,
        source: ClientParameterSource::RuntimeContext,
    });
    let request = ClientFrame::Request {
        base: base(),
        request_id: request_id("request"),
        catalog_generation: catalog.catalog_generation.clone(),
        workspace_generation: catalog.workspace_generation.clone(),
        method: "rust.search".to_owned(),
        params: json!({"query": "owner", "workspace": "../other"}),
    };
    let error = ClientSessionState::Initialized
        .admit(&request, &catalog)
        .expect_err("RuntimeContext injection must fail");
    assert_eq!(error.reason_kind, "client-runtime-context-injection-denied");
}

#[test]
fn shared_conformance_fixture_runs_in_the_reference_implementation() {
    let suite: ClientConformanceSuite = serde_json::from_str(include_str!(
        "../../../schemas/fixtures/asp-client-conformance/base.json"
    ))
    .expect("parse shared conformance suite");
    let receipts = run_conformance_suite(&suite, &catalog()).expect("conformance suite");
    assert_eq!(receipts.len(), 5);
    assert_eq!(
        receipts[2].reason_kind.as_deref(),
        Some("method-not-in-client-catalog")
    );
    assert_eq!(
        receipts[3].reason_kind.as_deref(),
        Some("client-catalog-generation-mismatch")
    );
}

#[test]
fn session_and_workspace_identity_are_bound_at_initialize() {
    let catalog = catalog();
    let mut session = ClientSession::default();
    session
        .admit(
            &ClientFrame::Initialize {
                base: base(),
                request_id: request_id("initialize"),
                project_root: "/workspace".to_owned(),
                client_info: ClientInfo {
                    name: "test".to_owned(),
                    version: "1".to_owned(),
                },
                capabilities: json!({}),
            },
            &catalog,
        )
        .expect("initialize");
    let mut crossed = base();
    crossed.workspace_identity =
        ClientWorkspaceIdentity::new("other-workspace").expect("workspace id");
    let error = session
        .admit(
            &ClientFrame::Request {
                base: crossed,
                request_id: request_id("request"),
                catalog_generation: catalog.catalog_generation.clone(),
                workspace_generation: catalog.workspace_generation.clone(),
                method: "rust.search".to_owned(),
                params: json!({"query": "owner"}),
            },
            &catalog,
        )
        .expect_err("workspace crossing must fail");
    assert_eq!(error.reason_kind, "client-session-isolation-mismatch");
}
