//! Typed `ClientFrame` transport for commands sent to an existing ASP Runtime Server.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use agent_semantic_client_protocol::{
    CLIENT_FRAME_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION, ClientFrame,
    ClientFrameBase, ClientInfo, ClientRequestId, ClientSessionId, ClientWorkspaceIdentity,
    GRAPH_EVALUATE_METHOD, SCHEMA_BUNDLE_METHOD, SCHEMA_BUNDLE_REQUEST_SCHEMA_ID, SCHEMA_VERSION,
    SchemaBundleRequest, SchemaBundleResponse,
};
use agent_semantic_client_server::AspClientGrpcTransport;

/// Monotonic request identity shared by all warm client sessions in a process.
static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// A multiplexed gRPC session pinned to one published Runtime endpoint.
///
/// The transport owns one response reader and can serve concurrent calls. The
/// initialization cell ensures the protocol handshake is sent once per
/// connection instead of once per query.
struct CachedClientSession {
    transport: Arc<AspClientGrpcTransport>,
    session_id: ClientSessionId,
    initialized: tokio::sync::OnceCell<()>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct SessionKey {
    socket_path: PathBuf,
    workspace_identity: String,
}

static SESSION_CACHE: OnceLock<tokio::sync::Mutex<HashMap<SessionKey, Arc<CachedClientSession>>>> =
    OnceLock::new();

fn session_cache() -> &'static tokio::sync::Mutex<HashMap<SessionKey, Arc<CachedClientSession>>> {
    SESSION_CACHE.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()))
}

fn request_id(prefix: &str) -> Result<ClientRequestId, String> {
    let sequence = REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    ClientRequestId::new(format!("{prefix}-{}-{sequence}", std::process::id()))
}

fn frame_base(
    session_id: ClientSessionId,
    workspace_identity: String,
) -> Result<ClientFrameBase, String> {
    Ok(ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id,
        workspace_identity: ClientWorkspaceIdentity::new(workspace_identity)?,
        trace_context: None,
    })
}

async fn session_for_endpoint(
    socket_path: &Path,
    workspace_identity: &str,
) -> Result<Arc<CachedClientSession>, String> {
    let key = SessionKey {
        socket_path: socket_path.to_owned(),
        workspace_identity: workspace_identity.to_owned(),
    };
    if let Some(session) = session_cache().lock().await.get(&key).cloned() {
        return Ok(session);
    }
    let transport = Arc::new(AspClientGrpcTransport::connect_unix(socket_path).await?);
    let session = Arc::new(CachedClientSession {
        transport,
        session_id: ClientSessionId::new(format!(
            "asp-client-{}-{}",
            std::process::id(),
            REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))?,
        initialized: tokio::sync::OnceCell::new(),
    });
    let mut cache = session_cache().lock().await;
    Ok(cache
        .entry(key)
        .or_insert_with(|| Arc::clone(&session))
        .clone())
}

async fn evict_session(key: &SessionKey, session: &Arc<CachedClientSession>) {
    let mut cache = session_cache().lock().await;
    if cache
        .get(key)
        .is_some_and(|current| Arc::ptr_eq(current, session))
    {
        cache.remove(key);
    }
}

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
        let socket_path = PathBuf::from(endpoint.data_plane_socket_path);
        let session_key = SessionKey {
            socket_path: socket_path.clone(),
            workspace_identity: workspace_identity.clone(),
        };
        let session = session_for_endpoint(&socket_path, &workspace_identity).await?;
        let project_root = self.project_root.display().to_string();
        let client_info = ClientInfo {
            name: "asp-client".to_owned(),
            version: "1".to_owned(),
        };
        let initialize_result = session
            .initialized
            .get_or_try_init(|| async {
                let base = frame_base(session.session_id.clone(), workspace_identity.clone())?;
                session
                    .transport
                    .call(ClientFrame::Initialize {
                        base,
                        request_id: request_id("initialize")?,
                        project_root: project_root.clone(),
                        client_info: client_info.clone(),
                        capabilities: serde_json::json!({"requestCancellation": true}),
                    })
                    .await
                    .map(|_| ())
            })
            .await;
        if let Err(error) = initialize_result {
            evict_session(&session_key, &session).await;
            return Err(error);
        }
        let base = frame_base(session.session_id.clone(), workspace_identity)?;
        let result = session
            .transport
            .call(ClientFrame::Dispatch {
                base,
                request_id: request_id("dispatch")?,
                project_root,
                client_info,
                method,
                params,
            })
            .await;
        if result.is_err() {
            evict_session(&session_key, &session).await;
        }
        result
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

    /// Evaluate a graph request through the server-owned ClientFrame route.
    ///
    /// Graph algorithms are an internal Runtime Server service; the client
    /// only submits the versioned JSON payload and decodes the typed result.
    pub async fn graphs_evaluate(
        &self,
        params: serde_json::Value,
    ) -> Result<agent_semantic_search_projection::GraphTurboResultPacketV1, String> {
        decode_graph_evaluation_response(
            self.dispatch_method(GRAPH_EVALUATE_METHOD.to_owned(), params)
                .await?,
        )
    }
}

pub(crate) fn decode_graph_evaluation_response(
    frame: ClientFrame,
) -> Result<agent_semantic_search_projection::GraphTurboResultPacketV1, String> {
    match frame {
        ClientFrame::Response {
            outcome: agent_semantic_client_protocol::ClientOutcome::Ready,
            result: Some(result),
            error: None,
            ..
        } => agent_semantic_search_projection::GraphTurboResultPacketV1::from_value(result)
            .map_err(|error| format!("decode graph evaluation response: {error}")),
        ClientFrame::Response { outcome, error, .. } => Err(format!(
            "graph evaluation dispatch failed: outcome={outcome:?} error={}",
            error.unwrap_or(serde_json::Value::Null)
        )),
        frame => Err(format!(
            "graph evaluation dispatch returned a non-response frame: {frame:?}"
        )),
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
