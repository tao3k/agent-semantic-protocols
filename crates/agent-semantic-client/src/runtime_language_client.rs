//! Typed `ClientFrame` transport for commands sent to an existing ASP Runtime Server.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use agent_semantic_client_db::runtime_server_control::RuntimeServerEndpoint;
use agent_semantic_client_protocol::{
    CLIENT_FRAME_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION, ClientFrame,
    ClientFrameBase, ClientInfo, ClientRequestId, ClientSessionId, ClientWorkspaceIdentity,
    GRAPH_EVALUATE_METHOD, GRAPH_TIMELINE_METHOD, SCHEMA_BUNDLE_METHOD,
    SCHEMA_BUNDLE_REQUEST_SCHEMA_ID, SCHEMA_VERSION, SchemaBundleRequest, SchemaBundleResponse,
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

const SESSION_REGISTRY_CAPACITY: usize = 32;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SessionKey {
    socket_path: PathBuf,
    workspace_identity: String,
    owner_epoch: u64,
    binding_token: String,
    runtime_generation_digest: String,
    binary_content_digest: String,
}

impl SessionKey {
    fn from_endpoint(endpoint: &RuntimeServerEndpoint, workspace_identity: String) -> Self {
        Self {
            socket_path: PathBuf::from(&endpoint.data_plane_socket_path),
            workspace_identity,
            owner_epoch: endpoint.owner_epoch,
            binding_token: endpoint.binding_token.clone(),
            runtime_generation_digest: endpoint.runtime_generation_digest.clone(),
            binary_content_digest: endpoint.binary_content_digest.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn fixture(identity: u64) -> Self {
        Self {
            socket_path: PathBuf::from(format!("/tmp/asp-runtime-language-{identity}.sock")),
            workspace_identity: format!("workspace-{identity}"),
            owner_epoch: identity + 1,
            binding_token: format!("binding-{identity}"),
            runtime_generation_digest: format!("blake3-256:{:064x}", identity + 1),
            binary_content_digest: format!("blake3-256:{:064x}", identity + 2),
        }
    }

    #[cfg(test)]
    pub(crate) fn fixture_successor(identity: u64) -> Self {
        let mut key = Self::fixture(identity);
        key.owner_epoch += 1;
        key.binding_token.push_str("-successor");
        key.runtime_generation_digest = format!("blake3-256:{}", "f".repeat(64));
        key
    }
}

pub(crate) struct SessionRegistry<T> {
    capacity: usize,
    entries: HashMap<SessionKey, Arc<tokio::sync::OnceCell<Arc<T>>>>,
    lru: VecDeque<SessionKey>,
}

impl<T> SessionRegistry<T> {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: HashMap::new(),
            lru: VecDeque::new(),
        }
    }

    fn touch(&mut self, key: &SessionKey) {
        if let Some(index) = self.lru.iter().position(|candidate| candidate == key) {
            self.lru.remove(index);
        }
        self.lru.push_back(key.clone());
    }

    fn evictable(cell: &tokio::sync::OnceCell<Arc<T>>) -> bool {
        cell.get()
            .is_some_and(|value| Arc::strong_count(value) == 1)
    }

    fn evict_one_idle(&mut self) -> bool {
        let Some(index) = self.lru.iter().position(|key| {
            self.entries
                .get(key)
                .is_some_and(|cell| Self::evictable(cell))
        }) else {
            return false;
        };
        let key = self.lru.remove(index).expect("LRU index was present");
        self.entries.remove(&key);
        true
    }

    pub(crate) fn reserve(
        &mut self,
        key: SessionKey,
    ) -> Result<Arc<tokio::sync::OnceCell<Arc<T>>>, String> {
        if let Some(cell) = self.entries.get(&key).cloned() {
            self.touch(&key);
            return Ok(cell);
        }
        while self.entries.len() >= self.capacity {
            if !self.evict_one_idle() {
                return Err(format!(
                    "reasonKind=runtime-client-session-capacity-exhausted capacity={} activeOrConnecting={}",
                    self.capacity,
                    self.entries.len()
                ));
            }
        }
        let cell = Arc::new(tokio::sync::OnceCell::new());
        self.entries.insert(key.clone(), Arc::clone(&cell));
        self.touch(&key);
        Ok(cell)
    }

    fn remove_if_same(&mut self, key: &SessionKey, expected: &Arc<tokio::sync::OnceCell<Arc<T>>>) {
        if self
            .entries
            .get(key)
            .is_some_and(|current| Arc::ptr_eq(current, expected))
        {
            self.entries.remove(key);
            if let Some(index) = self.lru.iter().position(|candidate| candidate == key) {
                self.lru.remove(index);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn drain_idle(&mut self) -> usize {
        let before = self.entries.len();
        while self.evict_one_idle() {}
        before - self.entries.len()
    }
}

static SESSION_REGISTRY: OnceLock<tokio::sync::Mutex<SessionRegistry<CachedClientSession>>> =
    OnceLock::new();

fn session_registry() -> &'static tokio::sync::Mutex<SessionRegistry<CachedClientSession>> {
    SESSION_REGISTRY
        .get_or_init(|| tokio::sync::Mutex::new(SessionRegistry::new(SESSION_REGISTRY_CAPACITY)))
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

pub(crate) async fn session_for_key<T, Create, CreateFuture>(
    registry: &tokio::sync::Mutex<SessionRegistry<T>>,
    key: &SessionKey,
    create: Create,
) -> Result<(Arc<T>, Arc<tokio::sync::OnceCell<Arc<T>>>), String>
where
    Create: FnOnce() -> CreateFuture,
    CreateFuture: std::future::Future<Output = Result<Arc<T>, String>>,
{
    let cell = registry.lock().await.reserve(key.clone())?;
    let value = cell.get_or_try_init(create).await.cloned();
    match value {
        Ok(value) => Ok((value, cell)),
        Err(error) => {
            registry.lock().await.remove_if_same(key, &cell);
            Err(error)
        }
    }
}

async fn session_for_endpoint(
    key: &SessionKey,
) -> Result<
    (
        Arc<CachedClientSession>,
        Arc<tokio::sync::OnceCell<Arc<CachedClientSession>>>,
    ),
    String,
> {
    session_for_key(session_registry(), key, || async {
        let transport = Arc::new(AspClientGrpcTransport::connect_unix(&key.socket_path).await?);
        Ok::<_, String>(Arc::new(CachedClientSession {
            transport,
            session_id: ClientSessionId::new(format!(
                "asp-client-{}-{}",
                std::process::id(),
                REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ))?,
            initialized: tokio::sync::OnceCell::new(),
        }))
    })
    .await
}

async fn evict_session(
    key: &SessionKey,
    cell: &Arc<tokio::sync::OnceCell<Arc<CachedClientSession>>>,
) {
    session_registry().lock().await.remove_if_same(key, cell);
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

    /// Drop every idle cached session during an explicit client drain.
    /// Active leases remain available until their in-flight calls complete.
    pub async fn drain_cached_sessions(&self) -> usize {
        session_registry().lock().await.drain_idle()
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

    pub(crate) async fn dispatch_method(
        &self,
        method: String,
        params: serde_json::Value,
    ) -> Result<ClientFrame, String> {
        let endpoint =
            match agent_semantic_client_db::read_runtime_server_endpoint(&self.state_home).await? {
                Some(endpoint) => endpoint,
                None => {
                    self.drain_cached_sessions().await;
                    return Err("ASP Server endpoint is unavailable".to_owned());
                }
            };
        endpoint.validate()?;
        let workspace_identity =
            agent_semantic_client_db::AgentSessionRegistry::workspace_id(&self.project_root)?;
        let session_key = SessionKey::from_endpoint(&endpoint, workspace_identity.clone());
        let (session, session_cell) = session_for_endpoint(&session_key).await?;
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
            evict_session(&session_key, &session_cell).await;
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
            evict_session(&session_key, &session_cell).await;
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

    /// Submit a history/timeline request to the resident ASP Server.  The
    /// client never starts or selects the Graph-Turbo executable directly.
    pub async fn graphs_timeline(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let frame = self
            .dispatch_method(GRAPH_TIMELINE_METHOD.to_owned(), params)
            .await?;
        match frame {
            ClientFrame::Response {
                outcome: agent_semantic_client_protocol::ClientOutcome::Ready,
                result: Some(result),
                error: None,
                ..
            } => Ok(result),
            ClientFrame::Response { outcome, error, .. } => Err(format!(
                "graph timeline dispatch failed: outcome={outcome:?} error={}",
                error.unwrap_or(serde_json::Value::Null)
            )),
            frame => Err(format!(
                "graph timeline dispatch returned a non-response frame: {frame:?}"
            )),
        }
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
