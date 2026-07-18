//! Synchronous public registry operations over the Turso-owned core.

use super::AgentSessionRegistry;
use super::storage::{
    block_on_agent_session_registry_async, turso_claim_resident_session, turso_delete_session,
    turso_query_sessions, turso_record_tool_event, turso_refresh_expired_sessions,
    turso_register_session, turso_session_by_id, turso_session_by_id_any_project,
    turso_session_by_name, turso_session_for_root_session_id_any_project,
    turso_set_archived_status, turso_update_session_status,
};
use crate::agent_session_registry::types::{
    AGENT_SESSION_STATUS_ACTIVE, AGENT_SESSION_STATUS_ARCHIVED, AGENT_SESSION_STATUS_INVALID,
    AgentSessionDispatchClaimRequest, AgentSessionDispatchClaimResult,
    AgentSessionDispatchCompleteRequest, AgentSessionDispatchLeaseRecord,
    AgentSessionLookupRequest, AgentSessionRecord, AgentSessionRegisterRequest,
    AgentSessionToolEventRequest, agent_session_unix_timestamp,
};

impl AgentSessionRegistry {
    pub fn register_session(
        &self,
        request: AgentSessionRegisterRequest<'_>,
    ) -> Result<AgentSessionRecord, String> {
        block_on_agent_session_registry_async(turso_register_session(&self.db_path, request))
    }

    /// Claim a resident route without replacing the child that already owns it.
    pub fn claim_resident_session(
        &self,
        request: AgentSessionRegisterRequest<'_>,
    ) -> Result<AgentSessionRecord, String> {
        block_on_agent_session_registry_async(turso_claim_resident_session(&self.db_path, request))
    }

    /// Return registered sessions for one project, optionally narrowed by root session and name.
    pub fn query_sessions(
        &self,
        project_id: &str,
        root_session_id: Option<&str>,
        name: Option<&str>,
    ) -> Result<Vec<AgentSessionRecord>, String> {
        block_on_agent_session_registry_async(turso_query_sessions(
            &self.db_path,
            project_id,
            root_session_id,
            name,
        ))
    }

    /// Return one registered session by its concrete session id.
    pub fn session_by_id(
        &self,
        project_id: &str,
        session_id: &str,
    ) -> Result<Option<AgentSessionRecord>, String> {
        block_on_agent_session_registry_async(turso_session_by_id(
            &self.db_path,
            project_id,
            session_id,
        ))
    }

    /// Return one registered session by its stable root/name route.
    pub fn session_by_name(
        &self,
        project_id: &str,
        root_session_id: &str,
        name: &str,
    ) -> Result<Option<AgentSessionRecord>, String> {
        block_on_agent_session_registry_async(turso_session_by_name(
            &self.db_path,
            project_id,
            root_session_id,
            name,
        ))
    }

    /// Record the latest tool event for one registered session.
    pub fn record_tool_event(
        &self,
        request: AgentSessionToolEventRequest<'_>,
    ) -> Result<bool, String> {
        block_on_agent_session_registry_async(turso_record_tool_event(&self.db_path, request))
    }

    /// Claim, poll, or rebind one exact resident-child dispatch identity.
    pub fn claim_dispatch(
        &self,
        request: AgentSessionDispatchClaimRequest<'_>,
    ) -> Result<AgentSessionDispatchClaimResult, String> {
        block_on_agent_session_registry_async(
            crate::agent_session_registry::dispatch::turso_claim_dispatch(&self.db_path, request),
        )
    }

    /// Record the terminal receipt for one exact resident-child dispatch identity.
    pub fn complete_dispatch(
        &self,
        request: AgentSessionDispatchCompleteRequest<'_>,
    ) -> Result<AgentSessionDispatchLeaseRecord, String> {
        block_on_agent_session_registry_async(
            crate::agent_session_registry::dispatch::turso_complete_dispatch(
                &self.db_path,
                request,
            ),
        )
    }

    /// Generic lookup used by registry CLI commands.
    pub fn lookup_session(
        &self,
        request: AgentSessionLookupRequest<'_>,
    ) -> Result<Option<AgentSessionRecord>, String> {
        if let Some(session_id) = request.session_id {
            return self.session_by_id(request.project_id, session_id);
        }
        if let (Some(root_session_id), Some(name)) = (request.root_session_id, request.name) {
            return self.session_by_name(request.project_id, root_session_id, name);
        }
        let sessions =
            self.query_sessions(request.project_id, request.root_session_id, request.name)?;
        Ok(sessions.into_iter().next())
    }

    /// Update one session row to the supplied routing status.
    pub fn update_session_status(
        &self,
        project_id: &str,
        session_id: &str,
        status: &str,
        now: i64,
    ) -> Result<bool, String> {
        block_on_agent_session_registry_async(turso_update_session_status(
            &self.db_path,
            project_id,
            session_id,
            status,
            now,
        ))
    }

    /// Mark one session row invalid.
    pub fn mark_session_invalid(
        &self,
        project_id: &str,
        session_id: &str,
        now: i64,
    ) -> Result<bool, String> {
        self.update_session_status(project_id, session_id, AGENT_SESSION_STATUS_INVALID, now)
    }

    /// Archive one session row.
    pub fn archive_session(
        &self,
        project_id: &str,
        session_id: &str,
        now: i64,
    ) -> Result<bool, String> {
        block_on_agent_session_registry_async(turso_set_archived_status(
            &self.db_path,
            project_id,
            session_id,
            AGENT_SESSION_STATUS_ARCHIVED,
            Some(now),
            now,
        ))
    }

    /// Unarchive one session row.
    pub fn unarchive_session(
        &self,
        project_id: &str,
        session_id: &str,
        now: i64,
    ) -> Result<bool, String> {
        block_on_agent_session_registry_async(turso_set_archived_status(
            &self.db_path,
            project_id,
            session_id,
            AGENT_SESSION_STATUS_ACTIVE,
            None,
            now,
        ))
    }

    /// Delete one session row.
    pub fn delete_session(&self, project_id: &str, session_id: &str) -> Result<bool, String> {
        block_on_agent_session_registry_async(turso_delete_session(
            &self.db_path,
            project_id,
            session_id,
        ))
    }

    /// Refresh expired routable sessions in this registry DB.
    pub fn refresh_expired_sessions(&self) -> Result<(), String> {
        let now = agent_session_unix_timestamp()?;
        block_on_agent_session_registry_async(turso_refresh_expired_sessions(&self.db_path, now))
    }

    /// Return one registered session by its concrete session id across all projects.
    pub fn session_by_id_any_project(
        &self,
        session_id: &str,
    ) -> Result<Option<AgentSessionRecord>, String> {
        block_on_agent_session_registry_async(turso_session_by_id_any_project(
            &self.db_path,
            session_id,
        ))
    }

    /// Return the project id for the most recent session registered under one root.
    pub fn project_id_for_root_session_id(
        &self,
        root_session_id: &str,
    ) -> Result<Option<String>, String> {
        Ok(
            block_on_agent_session_registry_async(turso_session_for_root_session_id_any_project(
                &self.db_path,
                root_session_id,
            ))?
            .map(|record| record.project_id),
        )
    }
}
