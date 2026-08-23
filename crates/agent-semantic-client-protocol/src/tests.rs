use serde_json::json;

use super::*;

fn digest(character: char) -> String {
    format!("blake3-256:{}", character.to_string().repeat(64))
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
        session_id: "session".to_owned(),
        workspace_identity: "workspace".to_owned(),
        trace_context: None,
    }
}

#[test]
fn catalog_is_closed_and_generation_pinned() {
    let catalog = catalog();
    catalog.validate().expect("valid catalog");
    let unknown = ClientFrame::Request {
        base: base(),
        request_id: "request".to_owned(),
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
        request_id: "initialize".to_owned(),
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
        request_id: "shutdown".to_owned(),
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
        request_id: "request".to_owned(),
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
        request_id: "request".to_owned(),
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
        request_id: "request".to_owned(),
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
                request_id: "initialize".to_owned(),
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
    crossed.workspace_identity = "other-workspace".to_owned();
    let error = session
        .admit(
            &ClientFrame::Request {
                base: crossed,
                request_id: "request".to_owned(),
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
