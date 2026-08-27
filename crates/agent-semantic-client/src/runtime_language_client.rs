//! Typed `ClientFrame` transport for commands sent to an existing ASP Runtime Server.

use std::path::PathBuf;

use agent_semantic_client_protocol::{
    CLIENT_FRAME_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION, ClientFrame,
    ClientFrameBase, ClientInfo, ClientRequestId, ClientSessionId, ClientWorkspaceIdentity,
    SCHEMA_BUNDLE_METHOD, SCHEMA_BUNDLE_REQUEST_SCHEMA_ID, SCHEMA_VERSION, SchemaBundleRequest,
    SchemaBundleResponse,
};
use agent_semantic_client_server::AspClientGrpcTransport;

/// ASP Client command transport to an already-published ASP Server endpoint.
pub struct AspClient {
    state_home: PathBuf,
    project_root: PathBuf,
}

impl AspClient {
    /// Create a command transport without starting or owning a Runtime session.
    pub fn new(state_home: impl Into<PathBuf>, project_root: impl Into<PathBuf>) -> Self {
        Self {
            state_home: state_home.into(),
            project_root: project_root.into(),
        }
    }

    /// Dispatch one typed method to the resident ASP Server.
    pub async fn dispatch(
        &self,
        language_id: &str,
        route: &str,
        params: serde_json::Value,
    ) -> Result<ClientFrame, String> {
        self.dispatch_method(format!("{language_id}.{route}"), params)
            .await
    }

    async fn dispatch_method(
        &self,
        method: String,
        params: serde_json::Value,
    ) -> Result<ClientFrame, String> {
        let endpoint = agent_semantic_client_db::read_runtime_server_endpoint(&self.state_home)
            .await?
            .ok_or_else(|| "ASP Server endpoint is unavailable".to_owned())?;
        endpoint.validate()?;
        let workspace_identity =
            agent_semantic_client_db::AgentSessionRegistry::workspace_id(&self.project_root)?;
        let transport =
            AspClientGrpcTransport::connect_unix(endpoint.data_plane_socket_path).await?;
        let process_id = std::process::id();
        let base = ClientFrameBase {
            schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
            protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
            session_id: ClientSessionId::new(format!("asp-client-{process_id}"))?,
            workspace_identity: ClientWorkspaceIdentity::new(workspace_identity)?,
            trace_context: None,
        };
        let project_root = self.project_root.display().to_string();
        let client_info = ClientInfo {
            name: "asp-client".to_owned(),
            version: "1".to_owned(),
        };
        transport
            .call(ClientFrame::Initialize {
                base: base.clone(),
                request_id: ClientRequestId::new(format!("initialize-{process_id}"))?,
                project_root: project_root.clone(),
                client_info: client_info.clone(),
                capabilities: serde_json::json!({"requestCancellation": true}),
            })
            .await?;
        transport
            .call(ClientFrame::Dispatch {
                base,
                request_id: ClientRequestId::new(format!("dispatch-{process_id}"))?,
                project_root,
                client_info,
                method,
                params,
            })
            .await
    }

    /// Fetch one canonical, Runtime-verified schema profile.
    pub async fn schema_bundle(
        &self,
        language_id: impl Into<String>,
        root_set_ids: Vec<String>,
        known_bundle_digest: Option<String>,
    ) -> Result<SchemaBundleResponse, String> {
        let request = SchemaBundleRequest {
            schema_id: SCHEMA_BUNDLE_REQUEST_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            language_id: language_id.into(),
            root_set_ids,
            known_bundle_digest,
        };
        request.validate()?;
        let params = serde_json::to_value(request)
            .map_err(|error| format!("encode schema bundle request: {error}"))?;
        decode_schema_bundle_response(
            self.dispatch_method(SCHEMA_BUNDLE_METHOD.to_owned(), params)
                .await?,
        )
    }
}

pub(crate) fn decode_schema_bundle_response(
    frame: ClientFrame,
) -> Result<SchemaBundleResponse, String> {
    match frame {
        ClientFrame::Response {
            outcome: agent_semantic_client_protocol::ClientOutcome::Ready,
            result: Some(result),
            error: None,
            ..
        } => {
            let response: SchemaBundleResponse = serde_json::from_value(result)
                .map_err(|error| format!("decode schema bundle response: {error}"))?;
            response.validate()?;
            Ok(response)
        }
        ClientFrame::Response { outcome, error, .. } => Err(format!(
            "schema bundle dispatch failed: outcome={outcome:?} error={}",
            error.unwrap_or(serde_json::Value::Null)
        )),
        frame => Err(format!(
            "schema bundle dispatch returned a non-response frame: {frame:?}"
        )),
    }
}
