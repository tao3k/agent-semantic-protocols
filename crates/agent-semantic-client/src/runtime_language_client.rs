//! Typed `ClientFrame` transport for commands sent to an existing ASP Runtime Server.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use agent_semantic_client_db::runtime_server_control::RuntimeServerEndpoint;
use agent_semantic_client_protocol::{
    CANCELLATION_PROBE_METHOD, CLIENT_FRAME_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION,
    ClientFrame, ClientFrameBase, ClientInfo, ClientProjectId, ClientProtocolCatalog,
    ClientRequestId, ClientSessionId, ClientWorkspaceIdentity, GRAPH_EVALUATE_METHOD,
    GRAPH_TIMELINE_METHOD, LIVE_CORPUS_CACHE_STATE_METHOD, LiveCorpusCacheStateReceipt,
    LiveCorpusCacheStateRequest, SCHEMA_BUNDLE_METHOD, SCHEMA_BUNDLE_REQUEST_SCHEMA_ID,
    SCHEMA_VERSION, SchemaBundleRequest, SchemaBundleResponse,
};
use agent_semantic_client_server::{
    AspClientGrpcTransport, CLIENT_FRAME_SESSION_CAPACITY, CLIENT_FRAME_SESSION_CONTROL_RESERVE,
};

/// Monotonic request identity shared by all warm client sessions in a process.
static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);
#[cfg(unix)]
static HOST_RUNTIME_DESCRIPTOR_CONSUMED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(unix)]
pub const ASP_RUNTIME_CLIENT_FD_ENV: &str = "ASP_RUNTIME_CLIENT_FD";

fn transport_unavailable(message: &str) -> String {
    format!("reasonKind=transport-unavailable {message}")
}

/// A multiplexed gRPC session pinned to one published Runtime endpoint.
///
/// The transport owns one response reader and can serve concurrent calls. The
/// initialization cell ensures the protocol handshake is sent once per
/// connection instead of once per query.
struct CachedClientSession {
    transport: Arc<AspClientGrpcTransport>,
    session_id: ClientSessionId,
    initialized: tokio::sync::OnceCell<ClientProtocolCatalog>,
}

const SESSION_REGISTRY_CAPACITY: usize = 32;
const CLIENT_REQUEST_CANCELLED_REASON_KIND: &str = "client-request-cancelled";

pub(crate) fn validate_cancelled_terminal(
    frame: ClientFrame,
    expected_request_id: &ClientRequestId,
) -> Result<(), String> {
    let ClientFrame::Response {
        request_id,
        outcome: agent_semantic_client_protocol::ClientOutcome::Cancelled,
        result: None,
        error: Some(error),
        catalog: None,
        ..
    } = frame
    else {
        return Err(
            "cancelled request must return one Cancelled response with typed diagnostic and no result or catalog"
                .to_owned(),
        );
    };
    if request_id != *expected_request_id {
        return Err(format!(
            "cancelled terminal request identity mismatch: expected={} actual={}",
            expected_request_id.as_str(),
            request_id.as_str(),
        ));
    }
    let diagnostic = error
        .as_object()
        .ok_or_else(|| "cancelled terminal error must be an object".to_owned())?;
    if diagnostic
        .keys()
        .any(|key| !matches!(key.as_str(), "reasonKind" | "message" | "terminal"))
    {
        return Err("cancelled terminal diagnostic contains unknown fields".to_owned());
    }
    if diagnostic
        .get("reasonKind")
        .and_then(serde_json::Value::as_str)
        != Some(CLIENT_REQUEST_CANCELLED_REASON_KIND)
    {
        return Err(format!(
            "cancelled terminal must carry reasonKind={CLIENT_REQUEST_CANCELLED_REASON_KIND}"
        ));
    }
    if !diagnostic
        .get("message")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|message| !message.trim().is_empty())
    {
        return Err("cancelled terminal diagnostic message must be non-empty".to_owned());
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SessionKey {
    project_id: String,
    workspace_id: String,
    binary_content_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientBackpressureProbeReceipt {
    pub capacity: usize,
    pub held_call_count: usize,
    pub rejected_call_count: usize,
    pub elapsed_micros: u64,
}

impl SessionKey {
    fn from_endpoint(
        endpoint: &RuntimeServerEndpoint,
        project_id: String,
        workspace_id: String,
    ) -> Self {
        Self {
            project_id,
            workspace_id,
            binary_content_digest: endpoint.binary_content_digest.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn fixture(identity: u64) -> Self {
        Self {
            project_id: format!("repo-{identity}"),
            workspace_id: format!("workspace-{identity}"),
            binary_content_digest: format!("blake3-256:{:064x}", identity + 2),
        }
    }

    #[cfg(test)]
    pub(crate) fn fixture_successor(identity: u64) -> Self {
        let mut key = Self::fixture(identity);
        key.binary_content_digest = format!("blake3-256:{}", "f".repeat(64));
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

    pub(crate) fn remove_key(&mut self, key: &SessionKey) -> bool {
        let removed = self.entries.remove(key).is_some();
        if let Some(index) = self.lru.iter().position(|candidate| candidate == key) {
            self.lru.remove(index);
        }
        removed
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
    project_id: &str,
    workspace_id: &str,
) -> Result<ClientFrameBase, String> {
    Ok(ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id,
        project_id: ClientProjectId::new(project_id)?,
        workspace_id: ClientWorkspaceIdentity::new(workspace_id)?,
        trace_context: None,
    })
}

fn project_workspace_ids(project_root: &std::path::Path) -> Result<(String, String), String> {
    let state = agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?;
    Ok((
        state.repo.repo_id.to_string(),
        state.workspace.workspace_id.to_string(),
    ))
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
    published_endpoint: std::net::SocketAddr,
    transport_capability: &AspClientTransportCapability,
) -> Result<
    (
        Arc<CachedClientSession>,
        Arc<tokio::sync::OnceCell<Arc<CachedClientSession>>>,
    ),
    String,
> {
    session_for_key(session_registry(), key, || async {
        let transport = Arc::new(match transport_capability {
            AspClientTransportCapability::PublishedLoopbackTcp => {
                AspClientGrpcTransport::connect_tcp(published_endpoint).await?
            }
            #[cfg(unix)]
            AspClientTransportCapability::InheritedDescriptor(descriptor) => {
                let descriptor = descriptor
                    .lock()
                    .map_err(|_| "inherited Runtime descriptor capability poisoned".to_owned())?
                    .take()
                    .ok_or_else(|| {
                        "reasonKind=transport-unavailable inherited Runtime descriptor capability was already consumed"
                            .to_owned()
                    })?;
                AspClientGrpcTransport::connect_inherited_descriptor(descriptor).await?
            }
        });
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
enum AspClientTransportCapability {
    PublishedLoopbackTcp,
    #[cfg(unix)]
    InheritedDescriptor(std::sync::Mutex<Option<std::os::fd::OwnedFd>>),
}

pub struct AspClient {
    state_home: PathBuf,
    project_root: PathBuf,
    transport_capability: AspClientTransportCapability,
}

impl AspClient {
    /// Create a command transport without starting or owning a Runtime session.
    pub fn new(state_home: impl Into<PathBuf>, project_root: impl Into<PathBuf>) -> Self {
        Self {
            state_home: state_home.into(),
            project_root: project_root.into(),
            transport_capability: AspClientTransportCapability::PublishedLoopbackTcp,
        }
    }

    /// Select the Host-published transport capability exactly once.
    ///
    /// Outside a restricted network sandbox, absence of
    /// `ASP_RUNTIME_CLIENT_FD` selects the published loopback TCP binding. A
    /// sandbox Host transfers an already connected descriptor and sets the
    /// variable to that descriptor number; malformed, closed, or replayed
    /// capabilities fail before socket I/O.
    #[cfg(unix)]
    pub fn new_from_host_capability(
        state_home: impl Into<PathBuf>,
        project_root: impl Into<PathBuf>,
    ) -> Result<Self, String> {
        let Ok(raw_descriptor) = std::env::var(ASP_RUNTIME_CLIENT_FD_ENV) else {
            return Ok(Self::new(state_home, project_root));
        };
        let raw_descriptor = raw_descriptor.parse::<std::os::fd::RawFd>().map_err(|_| {
            format!(
                "reasonKind=transport-unavailable {ASP_RUNTIME_CLIENT_FD_ENV} must be an open descriptor number"
            )
        })?;
        if raw_descriptor < 3 {
            return Err(format!(
                "reasonKind=transport-unavailable {ASP_RUNTIME_CLIENT_FD_ENV} must not alias stdin/stdout/stderr"
            ));
        }
        if HOST_RUNTIME_DESCRIPTOR_CONSUMED.swap(true, Ordering::AcqRel) {
            return Err(
                "reasonKind=transport-unavailable inherited Runtime descriptor capability was replayed"
                    .to_owned(),
            );
        }
        // SAFETY: the Host capability contract transfers sole ownership of an
        // open descriptor >= 3 to this process. F_GETFD verifies it is open
        // before OwnedFd assumes responsibility for closing it.
        if unsafe { libc::fcntl(raw_descriptor, libc::F_GETFD) } < 0 {
            return Err(format!(
                "reasonKind=transport-unavailable inherited Runtime descriptor is not open: {}",
                std::io::Error::last_os_error()
            ));
        }
        use std::os::fd::FromRawFd;
        // SAFETY: validated above and guarded against a second ownership claim.
        let descriptor = unsafe { std::os::fd::OwnedFd::from_raw_fd(raw_descriptor) };
        Ok(Self::new_with_inherited_descriptor(
            state_home,
            project_root,
            descriptor,
        ))
    }

    /// Create a sandbox-safe client from a Host-granted connected Runtime
    /// descriptor. The descriptor is consumed exactly once when the bounded
    /// gRPC session is created; failure never falls back to a pathname.
    #[cfg(unix)]
    pub fn new_with_inherited_descriptor(
        state_home: impl Into<PathBuf>,
        project_root: impl Into<PathBuf>,
        descriptor: std::os::fd::OwnedFd,
    ) -> Self {
        Self {
            state_home: state_home.into(),
            project_root: project_root.into(),
            transport_capability: AspClientTransportCapability::InheritedDescriptor(
                std::sync::Mutex::new(Some(descriptor)),
            ),
        }
    }

    /// Drop every idle cached session during an explicit client drain.
    /// Active leases remain available until their in-flight calls complete.
    pub async fn drain_cached_sessions(&self) -> usize {
        session_registry().lock().await.drain_idle()
    }

    /// Prepare one explicit Live Corpus cache state through the Runtime-owned
    /// cache authority.  Cold-load additionally evicts only this workspace's
    /// client session after the server confirms the exact generation/root.
    pub async fn prepare_live_corpus_cache_state(
        &self,
        request: LiveCorpusCacheStateRequest,
    ) -> Result<LiveCorpusCacheStateReceipt, String> {
        request.validate()?;
        let endpoint = agent_semantic_client_db::read_runtime_server_endpoint(&self.state_home)
            .await?
            .ok_or_else(|| transport_unavailable("ASP Server endpoint is unavailable"))?;
        endpoint.validate()?;
        let (project_id, workspace_id) = project_workspace_ids(&self.project_root)?;
        let session_key = SessionKey::from_endpoint(&endpoint, project_id, workspace_id);
        let cache_state = request.cache_state.clone();
        let frame = self
            .dispatch_method(
                LIVE_CORPUS_CACHE_STATE_METHOD.to_owned(),
                serde_json::to_value(request)
                    .map_err(|error| format!("encode Live Corpus cache-state request: {error}"))?,
            )
            .await?;
        let ClientFrame::Response {
            outcome: agent_semantic_client_protocol::ClientOutcome::Ready,
            result: Some(payload),
            error: None,
            ..
        } = frame
        else {
            return Err(format!(
                "Live Corpus cache-state request did not return Ready: {frame:?}"
            ));
        };
        let mut receipt = serde_json::from_value::<LiveCorpusCacheStateReceipt>(payload)
            .map_err(|error| format!("decode Live Corpus cache-state receipt: {error}"))?;
        receipt.validate()?;
        if receipt.project_id != session_key.project_id
            || receipt.workspace_id != session_key.workspace_id
        {
            return Err(
                "Live Corpus cache-state receipt crossed its ProjectId/WorkspaceId binding"
                    .to_owned(),
            );
        }
        if matches!(cache_state.as_str(), "cold-load" | "released") {
            receipt.client_session_evicted =
                session_registry().lock().await.remove_key(&session_key);
            if !receipt.client_session_evicted {
                return Err(format!(
                    "Live Corpus {cache_state} did not evict its exact client session"
                ));
            }
        }
        receipt.validate()?;
        Ok(receipt)
    }

    /// Dispatch one typed method to the resident ASP Server.
    pub async fn dispatch(
        &self,
        language_id: &str,
        route: &str,
        params: serde_json::Value,
    ) -> Result<ClientFrame, String> {
        self.dispatch_method_on_session(format!("{language_id}.{route}"), params)
            .await
    }

    pub(crate) async fn dispatch_method(
        &self,
        method: String,
        params: serde_json::Value,
    ) -> Result<ClientFrame, String> {
        self.dispatch_method_on_session(method, params).await
    }

    async fn initialized_session(
        &self,
    ) -> Result<
        (
            Arc<CachedClientSession>,
            Arc<tokio::sync::OnceCell<Arc<CachedClientSession>>>,
            ClientFrameBase,
            SessionKey,
        ),
        String,
    > {
        let endpoint =
            match agent_semantic_client_db::read_runtime_server_endpoint(&self.state_home).await? {
                Some(endpoint) => endpoint,
                None => {
                    self.drain_cached_sessions().await;
                    return Err(transport_unavailable("ASP Server endpoint is unavailable"));
                }
            };
        endpoint.validate()?;
        let (project_id, workspace_id) = project_workspace_ids(&self.project_root)?;
        let session_key =
            SessionKey::from_endpoint(&endpoint, project_id.clone(), workspace_id.clone());
        let (session, session_cell) = session_for_endpoint(
            &session_key,
            endpoint.data_endpoint.socket_addr(),
            &self.transport_capability,
        )
        .await?;
        let client_info = ClientInfo {
            name: "asp-client".to_owned(),
            version: "1".to_owned(),
        };
        let initialize_result = session
            .initialized
            .get_or_try_init(|| async {
                if matches!(
                    &self.transport_capability,
                    AspClientTransportCapability::PublishedLoopbackTcp
                ) {
                    agent_semantic_client_db::runtime_server_control::ensure_runtime_server_workspace(
                        &endpoint,
                        &self.project_root,
                        request_id("ensure-workspace")?.into_inner(),
                    )
                    .await?;
                }
                let base = frame_base(session.session_id.clone(), &project_id, &workspace_id)?;
                let terminal = session
                    .transport
                    .call(ClientFrame::Initialize {
                        base,
                        request_id: request_id("initialize")?,
                        client_info: client_info.clone(),
                        capabilities: serde_json::json!({"requestCancellation": true}),
                    })
                    .await?;
                let ClientFrame::Response {
                    outcome: agent_semantic_client_protocol::ClientOutcome::Ready,
                    catalog: Some(catalog),
                    result: None,
                    error: None,
                    ..
                } = terminal
                else {
                    return Err(format!(
                        "ASP Client initialization did not return Ready with catalog: {terminal:?}"
                    ));
                };
                Ok::<ClientProtocolCatalog, String>(catalog)
            })
            .await;
        if let Err(error) = initialize_result {
            evict_session(&session_key, &session_cell).await;
            return Err(error);
        }
        let base = frame_base(session.session_id.clone(), &project_id, &workspace_id)?;
        Ok((session, session_cell, base, session_key))
    }

    async fn dispatch_method_on_session(
        &self,
        method: String,
        params: serde_json::Value,
    ) -> Result<ClientFrame, String> {
        let (session, session_cell, base, session_key) = self.initialized_session().await?;
        let catalog = session
            .initialized
            .get()
            .expect("successful initialization publishes its catalog");
        let result = session
            .transport
            .call(ClientFrame::Request {
                base,
                request_id: request_id("dispatch")?,
                catalog_generation: catalog.catalog_generation.clone(),
                workspace_generation: catalog.workspace_generation.clone(),
                method,
                params,
            })
            .await;
        if result.is_err() {
            evict_session(&session_key, &session_cell).await;
        }
        result
    }

    /// Exercise the real request-cancellation path on the cached public gRPC
    /// session.  The probe dispatch remains pending until the correlated
    /// `Cancel` frame reaches the Runtime-owned cancellation registry.
    pub async fn cancellation_probe(&self) -> Result<u64, String> {
        let (session, session_cell, base, session_key) = self.initialized_session().await?;
        let catalog = session
            .initialized
            .get()
            .expect("successful initialization publishes its catalog");
        if !catalog.admits_method(CANCELLATION_PROBE_METHOD) {
            return Err("ASP Client catalog omitted the cancellation probe".to_owned());
        }
        let result = async {
            let cancellation_request_id = request_id("cancellation-probe")?;
            let started = tokio::time::Instant::now();
            let pending = session
                .transport
                .begin_call(ClientFrame::Request {
                    base: base.clone(),
                    request_id: cancellation_request_id.clone(),
                    catalog_generation: catalog.catalog_generation.clone(),
                    workspace_generation: catalog.workspace_generation.clone(),
                    method: CANCELLATION_PROBE_METHOD.to_owned(),
                    params: serde_json::json!({}),
                })
                .await?;
            session
                .transport
                .cancel_pending(base, cancellation_request_id.clone())
                .await?;
            validate_cancelled_terminal(pending.wait().await?, &cancellation_request_id)?;
            let residual = session.transport.pending_call_count();
            if residual != 0 {
                return Err(format!(
                    "ASP Client cancellation left residual pending calls: {residual}"
                ));
            }
            Ok(started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64)
        }
        .await;
        if result.is_err() {
            evict_session(&session_key, &session_cell).await;
        }
        result
    }

    /// Fill one real public gRPC session to its bounded data-call limit while
    /// retaining the reserved control slot, then prove that the next call is
    /// rejected deterministically and every admitted call can still receive a
    /// correlated Cancelled terminal.
    pub async fn backpressure_probe(&self) -> Result<ClientBackpressureProbeReceipt, String> {
        let endpoint = agent_semantic_client_db::read_runtime_server_endpoint(&self.state_home)
            .await?
            .ok_or_else(|| transport_unavailable("ASP Server endpoint is unavailable"))?;
        endpoint.validate()?;
        let (project_id, workspace_id) = project_workspace_ids(&self.project_root)?;
        let transport =
            AspClientGrpcTransport::connect_tcp(endpoint.data_endpoint.socket_addr()).await?;
        let session_id = ClientSessionId::new(format!(
            "asp-client-backpressure-probe-{}-{}",
            std::process::id(),
            REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))?;
        let client_info = ClientInfo {
            name: "asp-client".to_owned(),
            version: "1".to_owned(),
        };
        let base = frame_base(session_id, &project_id, &workspace_id)?;
        let initialized = transport
            .call(ClientFrame::Initialize {
                base: base.clone(),
                request_id: request_id("initialize")?,
                client_info: client_info.clone(),
                capabilities: serde_json::json!({"requestCancellation": true}),
            })
            .await?;
        let ClientFrame::Response {
            outcome: agent_semantic_client_protocol::ClientOutcome::Ready,
            catalog: Some(catalog),
            result: None,
            error: None,
            ..
        } = initialized
        else {
            return Err(format!(
                "ASP Client backpressure probe initialization did not return Ready with catalog: {initialized:?}"
            ));
        };
        if !catalog.admits_method(CANCELLATION_PROBE_METHOD) {
            return Err("ASP Client catalog omitted the cancellation probe".to_owned());
        }

        let started = tokio::time::Instant::now();
        let held_call_count = CLIENT_FRAME_SESSION_CAPACITY - CLIENT_FRAME_SESSION_CONTROL_RESERVE;
        let mut pending = Vec::with_capacity(held_call_count);
        for index in 0..held_call_count {
            let request_id = request_id(&format!("backpressure-held-{index}"))?;
            let call = transport
                .begin_call(ClientFrame::Request {
                    base: base.clone(),
                    request_id: request_id.clone(),
                    catalog_generation: catalog.catalog_generation.clone(),
                    workspace_generation: catalog.workspace_generation.clone(),
                    method: CANCELLATION_PROBE_METHOD.to_owned(),
                    params: serde_json::json!({}),
                })
                .await?;
            pending.push((request_id, call));
        }
        let rejected_request_id = request_id("backpressure-rejected")?;
        let rejected = match transport
            .begin_call(ClientFrame::Request {
                base: base.clone(),
                request_id: rejected_request_id,
                catalog_generation: catalog.catalog_generation.clone(),
                workspace_generation: catalog.workspace_generation.clone(),
                method: CANCELLATION_PROBE_METHOD.to_owned(),
                params: serde_json::json!({}),
            })
            .await
        {
            Ok(_) => {
                return Err(
                    "full client session admitted one excess data call without backpressure"
                        .to_owned(),
                );
            }
            Err(error) => error,
        };
        if !rejected.contains("reasonKind=client-session-backpressure")
            || !rejected.contains("retryAdmitted=false")
        {
            return Err(format!(
                "ASP Client backpressure probe returned an untyped rejection: {rejected}"
            ));
        }

        for (request_id, _) in &pending {
            transport
                .cancel_pending(base.clone(), request_id.clone())
                .await?;
        }
        for (request_id, call) in pending {
            validate_cancelled_terminal(call.wait().await?, &request_id)?;
        }
        if transport.pending_call_count() != 0 {
            return Err("ASP Client backpressure probe left residual pending calls".to_owned());
        }
        let shutdown = transport
            .call(ClientFrame::Shutdown {
                base: base.clone(),
                request_id: request_id("shutdown")?,
            })
            .await?;
        if !matches!(
            shutdown,
            ClientFrame::Response {
                outcome: agent_semantic_client_protocol::ClientOutcome::Ready,
                result: None,
                error: None,
                ..
            }
        ) {
            return Err(format!(
                "ASP Client backpressure probe shutdown did not return Ready: {shutdown:?}"
            ));
        }
        transport.send_oneway(ClientFrame::Exit { base }).await?;
        Ok(ClientBackpressureProbeReceipt {
            capacity: CLIENT_FRAME_SESSION_CAPACITY,
            held_call_count,
            rejected_call_count: 1,
            elapsed_micros: started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
        })
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

    /// Evaluate intent over the immutable resident graph bound by the Runtime.
    pub async fn graph_evaluate(
        &self,
        params: serde_json::Value,
    ) -> Result<agent_semantic_search_projection::ResidentGraphEvaluationResultV1, String> {
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
) -> Result<agent_semantic_search_projection::ResidentGraphEvaluationResultV1, String> {
    match frame {
        ClientFrame::Response {
            outcome: agent_semantic_client_protocol::ClientOutcome::Ready,
            result: Some(result),
            error: None,
            ..
        } => agent_semantic_search_projection::ResidentGraphEvaluationResultV1::from_value(result)
            .map_err(|error| format!("decode resident graph evaluation response: {error}")),
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
