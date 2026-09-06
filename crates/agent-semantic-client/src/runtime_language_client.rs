// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Typed `ClientFrame` transport for commands sent to an existing ASP Runtime Server.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use agent_semantic_client_protocol::CANCELLATION_PROBE_METHOD;
use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::ClientFrameBase;
use agent_semantic_client_protocol::ClientInfo;
use agent_semantic_client_protocol::ClientProjectId;
use agent_semantic_client_protocol::ClientProtocolCatalog;
use agent_semantic_client_protocol::ClientRequestId;
use agent_semantic_client_protocol::ClientSessionId;
use agent_semantic_client_protocol::ClientWorkspaceIdentity;
use agent_semantic_client_protocol::GRAPH_EVALUATE_METHOD;
use agent_semantic_client_protocol::GRAPH_TIMELINE_METHOD;
use agent_semantic_client_protocol::LIVE_CORPUS_CACHE_STATE_METHOD;
use agent_semantic_client_protocol::LiveCorpusCacheStateReceipt;
use agent_semantic_client_protocol::LiveCorpusCacheStateRequest;
use agent_semantic_client_protocol::SCHEMA_BUNDLE_METHOD;
use agent_semantic_client_protocol::SCHEMA_BUNDLE_REQUEST_SCHEMA_ID;
use agent_semantic_client_protocol::SchemaBundleRequest;
use agent_semantic_client_protocol::SchemaBundleResponse;
use agent_semantic_client_protocol::protocol_identity::CLIENT_FRAME_SCHEMA_ID;
use agent_semantic_client_protocol::protocol_identity::CLIENT_PROTOCOL_ID;
use agent_semantic_client_protocol::protocol_identity::CLIENT_PROTOCOL_VERSION;
use agent_semantic_client_protocol::protocol_identity::SCHEMA_VERSION;
use agent_semantic_client_server::AspClientGrpcTransport;
use agent_semantic_client_server::CLIENT_FRAME_SESSION_CAPACITY;
use agent_semantic_client_server::CLIENT_FRAME_SESSION_CONTROL_RESERVE;

/// Monotonic request identity shared by all warm client sessions in a process.
static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);
#[cfg(unix)]
static HOST_RUNTIME_DESCRIPTOR_CONSUMED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(unix)]
pub const ASP_RUNTIME_CLIENT_FD_ENV: &str = "ASP_RUNTIME_CLIENT_FD";

fn verified_runtime_endpoint_connect_failure(error: String) -> String {
    let lower = error.to_ascii_lowercase();
    if lower.contains("operation not permitted") || lower.contains("os error 1") {
        return format!(
            "reasonKind=host-operation-not-permitted failureLayer=runtime-verified-endpoint-transport osError=EPERM originalError={error}"
        );
    }
    let original = error
        .strip_prefix("reasonKind=transport-unavailable ")
        .unwrap_or(&error);
    format!(
        "reasonKind=runtime-verified-endpoint-connect-failed failureLayer=runtime-verified-endpoint-transport Runtime serving endpoint was content-proven but could not be connected: {original}"
    )
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
    publication_nonce: String,
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
    fn from_publication(
        publication: &AspClientRuntimeHandoff,
        project_id: String,
        workspace_id: String,
    ) -> Self {
        Self {
            project_id,
            workspace_id,
            publication_nonce: publication.publication_nonce.clone(),
            binary_content_digest: publication.artifact_digest.to_string(),
        }
    }

    #[cfg(test)]
    pub(crate) fn fixture(identity: u64) -> Self {
        Self {
            project_id: format!("repo-{identity}"),
            workspace_id: format!("workspace-{identity}"),
            publication_nonce: format!("publication-{identity}"),
            binary_content_digest: format!("blake3-256:{:064x}", identity + 2),
        }
    }

    #[cfg(test)]
    pub(crate) fn fixture_successor(identity: u64) -> Self {
        let mut key = Self::fixture(identity);
        key.publication_nonce = format!("publication-successor-{identity}");
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
                AspClientGrpcTransport::connect_tcp(published_endpoint)
                    .await
                    .map_err(verified_runtime_endpoint_connect_failure)?
            }
            #[cfg(unix)]
            AspClientTransportCapability::InheritedDescriptor(descriptor) => {
                let descriptor = descriptor
                    .lock()
                    .map_err(|_| "inherited Runtime descriptor capability poisoned".to_owned())?
                    .take()
                    .ok_or_else(|| {
                        "reasonKind=host-runtime-transport-capability-consumed failureLayer=host-transport-capability inherited Runtime descriptor capability was already consumed"
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

/// Immutable, content-bound transport authority minted by the Runtime's
/// resident transaction.  It deliberately excludes `activationGeneration`:
/// the publication nonce fences the transaction while the artifact digest
/// identifies the executable content served by the endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspClientRuntimeHandoff {
    control_socket_addr: std::net::SocketAddr,
    data_socket_addr: std::net::SocketAddr,
    provider_socket_addr: std::net::SocketAddr,
    publication_nonce: String,
    artifact_digest: agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    owner_epoch: u64,
    runtime_generation_digest: String,
}

impl AspClientRuntimeHandoff {
    pub fn control_socket_addr(&self) -> std::net::SocketAddr {
        self.control_socket_addr
    }

    pub fn data_socket_addr(&self) -> std::net::SocketAddr {
        self.data_socket_addr
    }

    pub fn provider_socket_addr(&self) -> std::net::SocketAddr {
        self.provider_socket_addr
    }

    pub fn publication_nonce(&self) -> &str {
        &self.publication_nonce
    }

    pub fn artifact_digest(
        &self,
    ) -> &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest {
        &self.artifact_digest
    }

    pub fn owner_epoch(&self) -> u64 {
        self.owner_epoch
    }

    pub fn runtime_generation_digest(&self) -> &str {
        &self.runtime_generation_digest
    }
}

impl
    TryFrom<
        &agent_semantic_client_db::runtime_server_owner_receipt::RuntimeServerResidentTransactionReceipt,
    > for AspClientRuntimeHandoff
{
    type Error = String;

    fn try_from(
        receipt: &agent_semantic_client_db::runtime_server_owner_receipt::RuntimeServerResidentTransactionReceipt,
    ) -> Result<Self, Self::Error> {
        const SCHEMA_ID: &str =
            "agent.semantic-protocols.runtime-server-resident-transaction-receipt";
        let identity_matches = receipt.schema_id == SCHEMA_ID
            && receipt.schema_version == "1"
            && receipt.state == "ready"
            && receipt.publication_nonce == receipt.applied_publication_nonce
            && receipt.launcher_artifact_digest == receipt.applied_artifact_digest
            && receipt.applied_artifact_digest == receipt.endpoint_binary_content_digest;
        if !identity_matches
            || receipt.publication_nonce.is_empty()
            || receipt.endpoint_owner_epoch == 0
            || !receipt
                .endpoint_runtime_generation_digest
                .strip_prefix("blake3-256:")
                .is_some_and(|digest| {
                    digest.len() == 64
                        && digest.bytes().all(|byte| {
                            byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
                        })
                })
        {
            return Err(
                "reasonKind=runtime-client-handoff-identity-mismatch resident transaction does not bind one exact Runtime publication"
                    .to_owned(),
            );
        }
        receipt.control_endpoint.validate()?;
        receipt.data_endpoint.validate()?;
        receipt.provider_endpoint.validate()?;
        Ok(Self {
            control_socket_addr: receipt.control_endpoint.socket_addr(),
            data_socket_addr: receipt.data_endpoint.socket_addr(),
            provider_socket_addr: receipt.provider_endpoint.socket_addr(),
            publication_nonce: receipt.publication_nonce.clone(),
            artifact_digest: receipt.endpoint_binary_content_digest.clone(),
            owner_epoch: receipt.endpoint_owner_epoch,
            runtime_generation_digest: receipt.endpoint_runtime_generation_digest.clone(),
        })
    }
}

pub struct AspClient {
    state_home: PathBuf,
    project_root: PathBuf,
    transport_capability: AspClientTransportCapability,
    runtime_handoff: Option<AspClientRuntimeHandoff>,
}

impl AspClient {
    /// Create a command transport without starting or owning a Runtime session.
    pub fn new(state_home: impl Into<PathBuf>, project_root: impl Into<PathBuf>) -> Self {
        Self {
            state_home: state_home.into(),
            project_root: project_root.into(),
            transport_capability: AspClientTransportCapability::PublishedLoopbackTcp,
            runtime_handoff: None,
        }
    }

    /// Create a client pinned to the exact Runtime publication returned by the
    /// resident transaction.  No State Home endpoint discovery is performed
    /// for this client.
    pub fn new_from_runtime_handoff(
        state_home: impl Into<PathBuf>,
        project_root: impl Into<PathBuf>,
        runtime_handoff: AspClientRuntimeHandoff,
    ) -> Self {
        Self {
            state_home: state_home.into(),
            project_root: project_root.into(),
            transport_capability: AspClientTransportCapability::PublishedLoopbackTcp,
            runtime_handoff: Some(runtime_handoff),
        }
    }

    pub(crate) fn with_runtime_handoff(mut self, runtime_handoff: AspClientRuntimeHandoff) -> Self {
        self.runtime_handoff = Some(runtime_handoff);
        self
    }

    async fn runtime_handoff(&self) -> Result<AspClientRuntimeHandoff, String> {
        if let Some(handoff) = &self.runtime_handoff {
            return Ok(handoff.clone());
        }
        let receipt = agent_semantic_client_db::runtime_server_lifecycle::observe_resident_transaction(
            &self.state_home,
        )
        .await
        .map_err(|error| {
            format!(
                "reasonKind=runtime-client-handoff-unavailable failureLayer=runtime-resident-transaction Runtime lifecycle authority did not return a content-bound serving capability: {error}"
            )
        })?;
        AspClientRuntimeHandoff::try_from(&receipt)
    }

    /// Select an optional Host-published transport capability exactly once.
    ///
    /// A descriptor is an additional Host capability, not the normal CLI
    /// transport authority.  Without it, the client resolves the Runtime's
    /// published endpoint through the State Home serving receipt before it
    /// attempts loopback I/O.  A supplied descriptor remains strict: malformed,
    /// closed, or replayed descriptors fail before socket I/O.
    #[cfg(unix)]
    pub fn new_from_host_capability(
        state_home: impl Into<PathBuf>,
        project_root: impl Into<PathBuf>,
    ) -> Result<Self, String> {
        let raw_descriptor = std::env::var(ASP_RUNTIME_CLIENT_FD_ENV).ok();
        Self::new_from_host_descriptor_value(state_home, project_root, raw_descriptor.as_deref())
    }

    /// Build the optional Host descriptor boundary from its already-read value.
    /// Keeping the parser independent from process environment makes the
    /// descriptor semantics deterministic and testable.
    pub(crate) fn new_from_host_descriptor_value(
        state_home: impl Into<PathBuf>,
        project_root: impl Into<PathBuf>,
        raw_descriptor: Option<&str>,
    ) -> Result<Self, String> {
        let Some(raw_descriptor) = raw_descriptor else {
            return Ok(Self::new(state_home, project_root));
        };
        let raw_descriptor = raw_descriptor.parse::<std::os::fd::RawFd>().map_err(|_| {
            format!(
                "reasonKind=host-runtime-transport-capability-invalid failureLayer=host-transport-capability {ASP_RUNTIME_CLIENT_FD_ENV} must be an open descriptor number"
            )
        })?;
        if raw_descriptor < 3 {
            return Err(format!(
                "reasonKind=host-runtime-transport-capability-invalid failureLayer=host-transport-capability {ASP_RUNTIME_CLIENT_FD_ENV} must not alias stdin/stdout/stderr"
            ));
        }
        if HOST_RUNTIME_DESCRIPTOR_CONSUMED.swap(true, Ordering::AcqRel) {
            return Err(
                "reasonKind=host-runtime-transport-capability-replayed failureLayer=host-transport-capability inherited Runtime descriptor capability was replayed"
                    .to_owned(),
            );
        }
        // SAFETY: the Host capability contract transfers sole ownership of an
        // open descriptor >= 3 to this process. F_GETFD verifies it is open
        // before OwnedFd assumes responsibility for closing it.
        if unsafe { libc::fcntl(raw_descriptor, libc::F_GETFD) } < 0 {
            return Err(format!(
                "reasonKind=host-runtime-transport-capability-invalid failureLayer=host-transport-capability inherited Runtime descriptor is not open: {}",
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
            runtime_handoff: None,
        }
    }

    pub(crate) fn uses_published_loopback_transport(&self) -> bool {
        matches!(
            self.transport_capability,
            AspClientTransportCapability::PublishedLoopbackTcp
        )
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
        let publication = self.runtime_handoff().await?;
        let (project_id, workspace_id) = project_workspace_ids(&self.project_root)?;
        let session_key = SessionKey::from_publication(&publication, project_id, workspace_id);
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
        let publication = match self.runtime_handoff().await {
            Ok(publication) => publication,
            Err(error) => {
                self.drain_cached_sessions().await;
                return Err(error);
            }
        };
        let (project_id, workspace_id) = project_workspace_ids(&self.project_root)?;
        let session_key =
            SessionKey::from_publication(&publication, project_id.clone(), workspace_id.clone());
        let (session, session_cell) = session_for_endpoint(
            &session_key,
            publication.data_socket_addr(),
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
        let endpoint = self.runtime_handoff().await?;
        let (project_id, workspace_id) = project_workspace_ids(&self.project_root)?;
        let transport = AspClientGrpcTransport::connect_tcp(endpoint.data_socket_addr())
            .await
            .map_err(verified_runtime_endpoint_connect_failure)?;
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
