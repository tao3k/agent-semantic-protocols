//! Server-owned, read-only agent-session projection for sandbox-safe control queries.

use std::sync::{Arc, RwLock};

use tokio::sync::watch;

#[derive(Clone)]
pub(crate) struct AgentSessionStatusHandle {
    value: Arc<RwLock<Vec<crate::runtime_server_control::RuntimeServerAgentSessionStatus>>>,
    generation: watch::Sender<u64>,
}

impl AgentSessionStatusHandle {
    pub(crate) fn new() -> Self {
        let (generation, _) = watch::channel(0);
        Self {
            value: Arc::new(RwLock::new(Vec::new())),
            generation,
        }
    }

    pub(crate) async fn refresh(
        &self,
        registry: &crate::AgentSessionRegistry,
    ) -> Result<(), String> {
        let now = crate::agent_session_unix_timestamp()?;
        let sessions = registry
            .query_all_sessions_local()
            .await?
            .into_iter()
            .map(|record| {
                let workspace_identity = crate::AgentSessionRegistry::workspace_id(
                    std::path::Path::new(record.project_id()),
                )?;
                let lifecycle_state = if record.expires_at().is_some_and(|expires| expires <= now) {
                    crate::runtime_server_control::RuntimeServerAgentSessionLifecycleState::Expired
                } else {
                    match record.status() {
                        crate::AGENT_SESSION_STATUS_ARCHIVED => {
                            crate::runtime_server_control::RuntimeServerAgentSessionLifecycleState::Archived
                        }
                        crate::AGENT_SESSION_STATUS_INVALID => {
                            crate::runtime_server_control::RuntimeServerAgentSessionLifecycleState::Invalid
                        }
                        _ if record.is_routable_at(now) => {
                            crate::runtime_server_control::RuntimeServerAgentSessionLifecycleState::Routable
                        }
                        _ => {
                            crate::runtime_server_control::RuntimeServerAgentSessionLifecycleState::Invalid
                        }
                    }
                };
                Ok(crate::runtime_server_control::RuntimeServerAgentSessionStatus {
                    workspace_identity,
                    project_id: record.project_id().to_owned(),
                    root_session_id: record.root_session_id().to_owned(),
                    session_id: record.session_id().to_owned(),
                    name: record.name().to_owned(),
                    physical_generation: u64::try_from(record.physical_generation).unwrap_or(0),
                    lifecycle_state,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let mut current = self
            .value
            .write()
            .map_err(|_| "Runtime Server agent-session status lock poisoned".to_owned())?;
        if *current != sessions {
            *current = sessions;
            self.generation.send_modify(|generation| {
                *generation = generation.saturating_add(1);
            });
        }
        Ok(())
    }

    pub(crate) fn shared(
        &self,
    ) -> Arc<RwLock<Vec<crate::runtime_server_control::RuntimeServerAgentSessionStatus>>> {
        Arc::clone(&self.value)
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.generation.subscribe()
    }
}
