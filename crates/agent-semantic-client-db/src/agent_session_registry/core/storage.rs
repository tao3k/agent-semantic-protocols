//! DB-owned storage for agent session registry rows.

use agent_semantic_client_core::state_core::ResolvedState;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use crate::engine::turso_statement::{execute_turso_operation, run_turso_operation};

use super::types::{AgentSessionRecord, AgentSessionRegisterRequest, AgentSessionToolEventRequest};
use crate::agent_session_registry::publication;
use crate::agent_session_registry::schema::bootstrap_turso_agent_session_schema;
pub(in crate::agent_session_registry) use crate::agent_session_registry::schema::{
    block_on_agent_session_registry_async, connect_turso_agent_session_registry,
};

const AGENT_SESSION_EXPIRED_REFRESH_LOCK_STALE_AFTER: Duration = Duration::from_secs(60);
static AGENT_SESSION_REGISTRY_RUNTIME_OWNER_PROCESS: AtomicBool = AtomicBool::new(false);

async fn runtime_server_endpoint_is_published(state_home: &Path) -> Result<bool, String> {
    Ok(crate::read_runtime_server_endpoint(state_home)
        .await?
        .is_some())
}

struct ExpiredRefreshLock {
    path: PathBuf,
}

impl Drop for ExpiredRefreshLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn expired_refresh_lock_path(db_path: &Path) -> PathBuf {
    db_path.with_extension("expired-refresh.lock")
}

fn try_acquire_expired_refresh_lock(db_path: &Path) -> Option<ExpiredRefreshLock> {
    let path = expired_refresh_lock_path(db_path);
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(_) => Some(ExpiredRefreshLock { path }),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let is_stale = fs::metadata(&path)
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|elapsed| elapsed > AGENT_SESSION_EXPIRED_REFRESH_LOCK_STALE_AFTER);
            if !is_stale {
                return None;
            }
            let _ = fs::remove_file(&path);
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .ok()
                .map(|_| ExpiredRefreshLock { path })
        }
        Err(_) => None,
    }
}

/// Turso-backed registry for agent session routing state.
pub struct AgentSessionRegistry {
    pub(super) db_path: PathBuf,
    pub(super) runtime_project_root: Option<PathBuf>,
}

impl AgentSessionRegistry {
    /// Mark the current dedicated Runtime Server process as the sole direct
    /// owner of the canonical agent-session Turso database.
    pub fn mark_runtime_server_owner_process() {
        AGENT_SESSION_REGISTRY_RUNTIME_OWNER_PROCESS.store(true, Ordering::Release);
    }

    pub(crate) fn is_runtime_server_owner_process() -> bool {
        AGENT_SESSION_REGISTRY_RUNTIME_OWNER_PROCESS.load(Ordering::Acquire)
    }

    /// Return the canonical identity of one concrete checkout/worktree workspace.
    pub fn workspace_id(project_root: impl AsRef<Path>) -> Result<String, String> {
        Ok(ResolvedState::resolve(project_root.as_ref())?
            .workspace
            .workspace_id
            .0)
    }

    /// Return the current checkout/worktree as a canonical workspace identity.
    pub fn current_workspace_id() -> Result<String, String> {
        let project_root = std::env::current_dir()
            .map_err(|error| format!("failed to read current directory: {error}"))?;
        Self::workspace_id(project_root)
    }

    /// Resolve a configured registry state root against the project root.
    pub fn resolve_state_root_override(
        project_root: impl AsRef<Path>,
        state_root: impl AsRef<Path>,
    ) -> PathBuf {
        if state_root.as_ref().is_absolute() {
            state_root.as_ref().to_path_buf()
        } else {
            project_root.as_ref().join(state_root.as_ref())
        }
    }

    #[must_use]
    pub fn state_root_for_resolved_state(state: &ResolvedState) -> PathBuf {
        state.state_home.clone()
    }

    pub fn state_root_for_project(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
        let state = ResolvedState::resolve(project_root.as_ref())?;
        Ok(Self::state_root_for_resolved_state(&state))
    }

    #[must_use]
    pub fn db_path_for_state_root(state_root: impl AsRef<Path>) -> PathBuf {
        publication::physical_current_db_path(state_root.as_ref())
    }

    pub async fn open_or_create_project(project_root: impl AsRef<Path>) -> Result<Self, String> {
        let project_root = project_root.as_ref();
        let state = ResolvedState::resolve(project_root)?;
        state.ensure_minimal_layout()?;
        if let Some(proxy) = Self::runtime_proxy(&state, project_root).await? {
            return Ok(proxy);
        }
        let endpoint_path = crate::runtime_server_endpoint_path(&state.state_home)?;
        Err(format!(
            "project registry create/open requires Runtime Server typed IPC; direct-open is forbidden: endpoint={} endpointPublished={} runtimeOwnerProcess={}",
            endpoint_path.display(),
            runtime_server_endpoint_is_published(&state.state_home).await?,
            AGENT_SESSION_REGISTRY_RUNTIME_OWNER_PROCESS.load(Ordering::Acquire)
        ))
    }

    pub async fn open_runtime_project_proxy(
        project_root: impl AsRef<Path>,
    ) -> Result<Option<Self>, String> {
        let project_root = project_root.as_ref();
        let state = ResolvedState::resolve(project_root)?;
        if let Some(proxy) = Self::runtime_proxy(&state, project_root).await? {
            return Ok(Some(proxy));
        }
        let endpoint_path = crate::runtime_server_endpoint_path(&state.state_home)?;
        if runtime_server_endpoint_is_published(&state.state_home).await? {
            return Err(format!(
                "read-only project registry direct-open is forbidden while Runtime Server endpoint is published: endpoint={} runtimeOwnerProcess={}",
                endpoint_path.display(),
                AGENT_SESSION_REGISTRY_RUNTIME_OWNER_PROCESS.load(Ordering::Acquire)
            ));
        }
        Ok(None)
    }

    pub async fn open_existing_project(
        project_root: impl AsRef<Path>,
    ) -> Result<Option<Self>, String> {
        let project_root = project_root.as_ref();
        let state = ResolvedState::resolve(project_root)?;
        if let Some(proxy) = Self::runtime_proxy(&state, project_root).await? {
            return Ok(Some(proxy));
        }
        let endpoint_path = crate::runtime_server_endpoint_path(&state.state_home)?;
        Err(format!(
            "project registry read requires Runtime Server typed IPC; direct-open is forbidden: endpoint={} endpointPublished={} runtimeOwnerProcess={}",
            endpoint_path.display(),
            runtime_server_endpoint_is_published(&state.state_home).await?,
            AGENT_SESSION_REGISTRY_RUNTIME_OWNER_PROCESS.load(Ordering::Acquire)
        ))
    }

    #[track_caller]
    pub fn open_or_create_state_root(state_root: impl AsRef<Path>) -> Result<Self, String> {
        fs::create_dir_all(state_root.as_ref()).map_err(|error| {
            format!(
                "failed to create agent session state root `{}`: {error}",
                state_root.as_ref().display()
            )
        })?;
        let db_path = block_on_agent_session_registry_async(
            publication::ensure_current_registry_published(state_root.as_ref()),
        )?;
        let registry = Self::open_path(&db_path).map_err(|error| {
            let caller = std::panic::Location::caller();
            format!(
                "direct state-root registry open failed at {}:{}: {error}",
                caller.file(),
                caller.line()
            )
        })?;
        registry.ensure_schema()?;
        Ok(registry)
    }

    /// Open the Runtime Server-owned registry without crossing a synchronous
    /// `block_on` bridge. Daemon bootstrap already runs inside the server's
    /// Tokio runtime, so schema initialization must remain in that lifecycle.
    pub async fn open_or_create_state_root_async(
        state_root: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let state_root = state_root.as_ref();
        tokio::fs::create_dir_all(state_root)
            .await
            .map_err(|error| {
                format!(
                    "failed to create agent session state root `{}`: {error}",
                    state_root.display()
                )
            })?;
        let registry = Self {
            db_path: publication::ensure_current_registry_published(state_root).await?,
            runtime_project_root: None,
        };
        bootstrap_turso_agent_session_schema(&registry.db_path).await?;
        Ok(registry)
    }

    pub async fn open_existing_state_root(
        state_root: impl AsRef<Path>,
    ) -> Result<Option<Self>, String> {
        let Some(db_path) = publication::read_current_registry_path(state_root.as_ref())? else {
            return Ok(None);
        };
        let registry = Self::open_path(&db_path).map_err(|error| {
            let caller = std::panic::Location::caller();
            format!(
                "direct existing state-root registry open failed at {}:{}: {error}",
                caller.file(),
                caller.line()
            )
        })?;
        registry.ensure_schema()?;
        registry.refresh_expired_sessions().await?;
        Ok(Some(registry))
    }

    pub fn open_existing_state_root_read_only(
        state_root: impl AsRef<Path>,
    ) -> Result<Option<Self>, String> {
        let Some(db_path) = publication::read_current_registry_path(state_root.as_ref())? else {
            return Ok(None);
        };
        Ok(Some(Self {
            db_path,
            runtime_project_root: None,
        }))
    }

    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    fn open_path(db_path: &Path) -> Result<Self, String> {
        let registry = Self {
            db_path: db_path.to_path_buf(),
            runtime_project_root: None,
        };
        registry.ensure_schema()?;
        Ok(registry)
    }

    async fn runtime_proxy(
        state: &ResolvedState,
        project_root: &Path,
    ) -> Result<Option<Self>, String> {
        if !runtime_server_endpoint_is_published(&state.state_home).await? {
            return Ok(None);
        }
        let project_root = fs::canonicalize(project_root).map_err(|error| {
            format!(
                "failed to canonicalize agent-session Runtime Server project root {}: {error}",
                project_root.display()
            )
        })?;
        // A client-side proxy is bound by the published Runtime endpoint, not by a
        // locally observable registry publication. The physical registry path is
        // Runtime-owned and may legitimately be unpublished while the resident
        // data plane is healthy. Requiring that path here made SubagentStart fall
        // back to the forbidden direct-open branch before it could issue the typed
        // Runtime IPC registration.
        let db_path = Self::db_path_for_state_root(&state.state_home);
        Ok(Some(Self {
            db_path,
            runtime_project_root: Some(project_root),
        }))
    }

    pub(in crate::agent_session_registry) async fn runtime_operation_async(
        &self,
        operation: crate::workspace_db_ipc::AgentSessionRegistryIpcOperation,
    ) -> Result<crate::workspace_db_ipc::AgentSessionRegistryIpcResult, String> {
        let project_root = self.runtime_project_root.as_ref().ok_or_else(|| {
            "session control-plane operations require the Runtime Server registry proxy".to_owned()
        })?;
        let session =
            crate::workspace_db_ipc::connect_runtime_server_workspace_session(project_root).await?;
        session.call_agent_session_registry(operation).await
    }

    pub(in crate::agent_session_registry) async fn runtime_operation(
        &self,
        operation: crate::workspace_db_ipc::AgentSessionRegistryIpcOperation,
    ) -> Result<Option<crate::workspace_db_ipc::AgentSessionRegistryIpcResult>, String> {
        let Some(project_root) = self.runtime_project_root.clone() else {
            let runtime_endpoint_present = match self.db_path.parent() {
                Some(state_home) => runtime_server_endpoint_is_published(state_home).await?,
                None => false,
            };
            if runtime_endpoint_present
                && !AGENT_SESSION_REGISTRY_RUNTIME_OWNER_PROCESS.load(Ordering::Acquire)
            {
                return Err(format!(
                    "direct agent-session registry access is forbidden while the Runtime Server endpoint is published: operation={operation:?}"
                ));
            }
            return Ok(None);
        };
        block_on_agent_session_registry_async(async move {
            let session =
                crate::workspace_db_ipc::connect_runtime_server_workspace_session(&project_root)
                    .await?;
            session
                .call_agent_session_registry(operation)
                .await
                .map(Some)
        })
    }

    fn ensure_schema(&self) -> Result<(), String> {
        block_on_agent_session_registry_async(bootstrap_turso_agent_session_schema(&self.db_path))
    }
}

pub(super) async fn turso_register_session(
    db_path: &Path,
    request: AgentSessionRegisterRequest<'_>,
) -> Result<AgentSessionRecord, String> {
    turso_register_session_once(db_path, &request).await
}

async fn turso_register_session_once(
    db_path: &Path,
    request: &AgentSessionRegisterRequest<'_>,
) -> Result<AgentSessionRecord, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    execute_turso_operation(
        || async {
            connection
                .execute(
                    "INSERT INTO asp_agent_sessions (
        project_id,
        root_session_id,
        session_id,
                message_target_id,
                parent_session_id,
                name,
                role,
                model,
                model_observation_source,
                model_observed_at,
                model_evidence_ref,
                status,
                created_at,
                updated_at,
                last_seen_at,
                last_heartbeat_at,
                expires_at,
                metadata_json,
                configured_agent_type,
                profile_evidence_json
    ) VALUES (
        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13, ?13, ?13, ?14, ?15,
        CASE WHEN json_valid(?15) AND json_extract(?15, '$.event') = 'subagent-start' AND json_extract(?15, '$.native') = 1 THEN json_extract(?15, '$.agentType') END,
        CASE WHEN json_valid(?15) AND json_extract(?15, '$.event') = 'subagent-start' AND json_extract(?15, '$.native') = 1 THEN ?15 END
    )
    ON CONFLICT(session_id) DO UPDATE SET
                message_target_id = excluded.message_target_id,
                model = CASE
                    WHEN excluded.model IS NOT NULL
                     AND (asp_agent_sessions.model_observed_at IS NULL
                          OR excluded.model_observed_at >= asp_agent_sessions.model_observed_at)
                    THEN excluded.model ELSE asp_agent_sessions.model END,
                model_observation_source = CASE
                    WHEN excluded.model IS NOT NULL
                     AND (asp_agent_sessions.model_observed_at IS NULL
                          OR excluded.model_observed_at >= asp_agent_sessions.model_observed_at)
                    THEN excluded.model_observation_source ELSE asp_agent_sessions.model_observation_source END,
                model_observed_at = CASE
                    WHEN excluded.model IS NOT NULL
                     AND (asp_agent_sessions.model_observed_at IS NULL
                          OR excluded.model_observed_at >= asp_agent_sessions.model_observed_at)
                    THEN excluded.model_observed_at ELSE asp_agent_sessions.model_observed_at END,
                model_evidence_ref = CASE
                    WHEN excluded.model IS NOT NULL
                     AND (asp_agent_sessions.model_observed_at IS NULL
                          OR excluded.model_observed_at >= asp_agent_sessions.model_observed_at)
                    THEN excluded.model_evidence_ref ELSE asp_agent_sessions.model_evidence_ref END,
                status = excluded.status,
                updated_at = excluded.updated_at,
                last_seen_at = excluded.last_seen_at,
                last_heartbeat_at = excluded.last_heartbeat_at,
                expires_at = excluded.expires_at,
                metadata_json = excluded.metadata_json,
                configured_agent_type = COALESCE(excluded.configured_agent_type, asp_agent_sessions.configured_agent_type),
                profile_evidence_json = COALESCE(excluded.profile_evidence_json, asp_agent_sessions.profile_evidence_json)
        WHERE asp_agent_sessions.project_id = excluded.project_id
          AND asp_agent_sessions.root_session_id = excluded.root_session_id
          AND asp_agent_sessions.parent_session_id IS excluded.parent_session_id
          AND asp_agent_sessions.name = excluded.name
          AND asp_agent_sessions.role = excluded.role",
                    (
                        &request.project_id,
                        &request.root_session_id,
                        &request.session_id,
                        request.message_target_id.as_ref(),
                        request.parent_session_id.as_ref(),
                        &request.name,
                        &request.role,
                        request.model_observation.as_ref().map(|observation| observation.model),
                        request
                            .model_observation
                            .as_ref()
                            .map(|observation| observation.source.as_str()),
                        request
                            .model_observation
                            .as_ref()
                            .map(|observation| observation.observed_at),
                        request
                            .model_observation
                            .as_ref()
                            .and_then(|observation| observation.evidence_ref),
                        &request.status,
                        request.now,
                        request.expires_at,
                        &request.metadata_json,
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to register Turso session",
    )
    .await?;
    let registered = match turso_session_by_id(
        db_path,
        request.project_id.as_str(),
        request.session_id.as_str(),
    )
    .await?
    {
        Some(registered) => registered,
        None => {
            let stored = turso_session_by_id_any_project(db_path, request.session_id.as_str())
                .await?
                .ok_or_else(|| "registered Turso session was not readable".to_string())?;
            return Err(format!(
                "agent-session-child-identity-rebind-denied: child={} storedProject={} requestedProject={}",
                request.session_id,
                stored.project_id(),
                request.project_id,
            ));
        }
    };
    if registered.root_session_id() != request.root_session_id
        || registered.parent_session_id() != request.parent_session_id.as_deref()
        || registered.name() != request.name
        || registered.role() != request.role
    {
        return Err(format!(
            "agent-session-child-identity-rebind-denied: child={} storedRoot={} requestedRoot={} storedParent={:?} requestedParent={:?} storedRoute={} requestedRoute={} storedRole={} requestedRole={}",
            request.session_id,
            registered.root_session_id(),
            request.root_session_id,
            registered.parent_session_id(),
            request.parent_session_id.as_deref(),
            registered.name(),
            request.name,
            registered.role(),
            request.role,
        ));
    }
    Ok(registered)
}

pub(in crate::agent_session_registry) async fn turso_session_by_name(
    db_path: &Path,
    project_id: &str,
    root_session_id: &str,
    name: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let sql =
        super::record::select_sql("WHERE project_id = ?1 AND root_session_id = ?2 AND name = ?3");
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(&sql, (project_id, root_session_id, name))
                .await
                .map_err(|error| error.to_string())
        },
        "failed to read Turso session by name",
    )
    .await?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso session by name row: {error}"))?
    else {
        return Ok(None);
    };
    let record = super::record::from_turso_row(&row)?;
    if rows
        .next()
        .await
        .map_err(|error| format!("failed to detect ambiguous Turso session route: {error}"))?
        .is_some()
    {
        return Err(format!(
            "agent-session-route-ambiguous: projectId={project_id} rootSessionId={root_session_id} name={name}; select one concrete childSessionId"
        ));
    }
    Ok(Some(record))
}

pub(super) async fn turso_query_sessions(
    db_path: &Path,
    project_id: &str,
    root_session_id: Option<&str>,
    name: Option<&str>,
) -> Result<Vec<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let sql = match (root_session_id, name) {
        (Some(_), Some(_)) => super::record::select_sql(
            "WHERE project_id = ?1 AND root_session_id = ?2 AND name = ?3 ORDER BY updated_at DESC, session_id",
        ),
        (Some(_), None) => super::record::select_sql(
            "WHERE project_id = ?1 AND root_session_id = ?2 ORDER BY updated_at DESC, session_id",
        ),
        (None, Some(_)) => super::record::select_sql(
            "WHERE project_id = ?1 AND name = ?2 ORDER BY updated_at DESC, session_id",
        ),
        (None, None) => {
            super::record::select_sql("WHERE project_id = ?1 ORDER BY updated_at DESC, session_id")
        }
    };
    let mut rows = match (root_session_id, name) {
        (Some(root_session_id), Some(name)) => {
            run_turso_operation(
                || async {
                    connection
                        .query(&sql, (project_id, root_session_id, name))
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to query Turso sessions",
            )
            .await?
        }
        (Some(root_session_id), None) => {
            run_turso_operation(
                || async {
                    connection
                        .query(&sql, (project_id, root_session_id))
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to query Turso sessions",
            )
            .await?
        }
        (None, Some(name)) => {
            run_turso_operation(
                || async {
                    connection
                        .query(&sql, (project_id, name))
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to query Turso sessions",
            )
            .await?
        }
        (None, None) => {
            run_turso_operation(
                || async {
                    connection
                        .query(&sql, [project_id])
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to query Turso sessions",
            )
            .await?
        }
    };
    let mut records = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso session row: {error}"))?
    {
        let record = super::record::from_turso_row(&row)?;
        if record.project_id() == project_id
            && root_session_id.is_none_or(|root| record.root_session_id() == root)
            && name.is_none_or(|route| record.name() == route)
        {
            records.push(record);
        }
    }
    Ok(records)
}

pub(super) async fn turso_query_all_sessions(
    db_path: &Path,
) -> Result<Vec<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let sql = super::record::select_sql("ORDER BY project_id, root_session_id, name");
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(&sql, ())
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query all Turso sessions for Runtime status projection",
    )
    .await?;
    let mut records = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Runtime status session projection row: {error}"))?
    {
        records.push(super::record::from_turso_row(&row)?);
    }
    Ok(records)
}

pub(in crate::agent_session_registry) async fn turso_session_by_id(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let sql = super::record::select_sql("WHERE project_id = ?1 AND session_id = ?2");
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(&sql, (project_id, session_id))
                .await
                .map_err(|error| error.to_string())
        },
        "failed to read Turso session by id",
    )
    .await?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso session by id row: {error}"))?
    else {
        return Ok(None);
    };
    super::record::from_turso_row(&row).map(Some)
}

pub(super) async fn turso_session_by_id_any_project(
    db_path: &Path,
    session_id: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let sql = super::record::select_sql("WHERE session_id = ?1 ORDER BY updated_at DESC LIMIT 1");
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(&sql, (session_id,))
                .await
                .map_err(|error| error.to_string())
        },
        "failed to read Turso session by id across projects",
    )
    .await?;
    let Some(row) = rows.next().await.map_err(|error| {
        format!("failed to read Turso session by id across projects row: {error}")
    })?
    else {
        return Ok(None);
    };
    super::record::from_turso_row(&row).map(Some)
}

pub(super) async fn turso_session_for_root_session_id_any_project(
    db_path: &Path,
    root_session_id: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let sql =
        super::record::select_sql("WHERE root_session_id = ?1 ORDER BY updated_at DESC LIMIT 1");
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(&sql, (root_session_id,))
                .await
                .map_err(|error| error.to_string())
        },
        "failed to read Turso session by root session id across projects",
    )
    .await?;
    let Some(row) = rows.next().await.map_err(|error| {
        format!("failed to read Turso session by root session id across projects row: {error}")
    })?
    else {
        return Ok(None);
    };
    super::record::from_turso_row(&row).map(Some)
}

pub(super) async fn turso_record_tool_event(
    db_path: &Path,
    request: AgentSessionToolEventRequest,
) -> Result<bool, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let updated = execute_turso_operation(
        || async {
            connection
                .execute(
                    "UPDATE asp_agent_sessions
                     SET last_tool_event = ?1,
                         last_command = ?2,
                         last_evidence_ref = ?3,
                         updated_at = ?4,
                         last_seen_at = ?4
                     WHERE session_id = ?5",
                    (
                        &request.tool_event,
                        request.command.as_ref(),
                        request.evidence_ref.as_ref(),
                        request.now,
                        &request.session_id,
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to record Turso session tool event",
    )
    .await?;
    Ok(updated > 0)
}

pub(super) async fn turso_update_session_status(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
    status: &str,
    now: i64,
) -> Result<bool, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let changes = execute_turso_operation(
        || async {
            connection
                .execute(
                    "UPDATE asp_agent_sessions
                     SET status = ?1,
                         updated_at = ?2,
                         last_seen_at = ?2
                     WHERE project_id = ?3 AND session_id = ?4",
                    (status, now, project_id, session_id),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to update Turso session status",
    )
    .await?;
    Ok(changes > 0)
}

pub(super) async fn turso_set_archived_status(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
    status: &str,
    archived_at: Option<i64>,
    now: i64,
) -> Result<bool, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let changes = execute_turso_operation(
        || async {
            connection
                .execute(
                    "UPDATE asp_agent_sessions
                     SET status = ?1,
                         archived_at = ?2,
                         updated_at = ?3,
                         last_seen_at = ?3
                     WHERE project_id = ?4 AND session_id = ?5",
                    (status, archived_at, now, project_id, session_id),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to update Turso session archive status",
    )
    .await?;
    Ok(changes > 0)
}

pub(super) async fn turso_delete_session(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
) -> Result<bool, String> {
    super::retirement::turso_retire_and_delete_session(db_path, project_id, session_id).await
}

pub(super) async fn turso_session_is_retired(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
) -> Result<bool, String> {
    super::retirement::turso_session_is_retired(db_path, project_id, session_id).await
}

pub(super) async fn turso_refresh_expired_sessions(db_path: &Path, now: i64) -> Result<(), String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let mut expired_rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT 1
             FROM asp_agent_sessions
             WHERE expires_at IS NOT NULL
               AND expires_at <= ?1
               AND status IN ('active', 'idle')
             LIMIT 1",
                    [now],
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to check Turso expired session rows",
    )
    .await?;
    let has_expired_rows = expired_rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso expired session row: {error}"))?
        .is_some();
    drop(expired_rows);
    if !has_expired_rows {
        return Ok(());
    }

    let Some(_refresh_lock) = try_acquire_expired_refresh_lock(db_path) else {
        return Ok(());
    };

    execute_turso_operation(
        || async {
            connection
                .execute(
                    "UPDATE asp_agent_sessions
             SET status = 'expired', updated_at = ?1
             WHERE expires_at IS NOT NULL
               AND expires_at <= ?1
               AND status IN ('active', 'idle')",
                    [now],
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to refresh Turso expired session rows",
    )
    .await?;
    Ok(())
}
