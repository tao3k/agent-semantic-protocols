// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Server-owned, read-only agent-session projection for sandbox-safe control queries.

use std::sync::{Arc, RwLock};

use tokio::sync::watch;

#[derive(Clone)]
pub(crate) struct AgentSessionStatusHandle {
    value: Arc<RwLock<Vec<crate::runtime_server_control::RuntimeServerAgentSessionStatus>>>,
    generation: watch::Sender<u64>,
    published_generation: watch::Sender<u64>,
}

impl AgentSessionStatusHandle {
    pub(crate) fn new() -> Self {
        let (generation, _) = watch::channel(0);
        let (published_generation, _) = watch::channel(0);
        Self {
            value: Arc::new(RwLock::new(Vec::new())),
            generation,
            published_generation,
        }
    }

    pub(crate) async fn refresh(
        &self,
        registry: &crate::AgentSessionRegistry,
    ) -> Result<u64, String> {
        let now = crate::agent_session_unix_timestamp()?;
        let sessions = registry
            .query_all_sessions_local()
            .await?
            .into_iter()
            .map(|record| {
                let workspace_identity = record.project_id().to_owned();
                let lifecycle_state = if record.expires_at().is_some_and(|expires| expires <= now) {
                    crate::runtime_server_control::RuntimeServerAgentSessionLifecycleState::Stopped
                } else {
                    match record.status() {
                        "stopped" => {
                            crate::runtime_server_control::RuntimeServerAgentSessionLifecycleState::Stopped
                        }
                        "achieved" => {
                            crate::runtime_server_control::RuntimeServerAgentSessionLifecycleState::Achieved
                        }
                        crate::AGENT_SESSION_STATUS_ARCHIVED => {
                            crate::runtime_server_control::RuntimeServerAgentSessionLifecycleState::Stopped
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
                let mut host_binding = serde_json::from_str::<serde_json::Value>(
                    record.metadata_json(),
                )
                .ok()
                .and_then(|metadata| metadata.get("hostBinding").cloned());
                if let Some(binding) = host_binding.as_mut().and_then(serde_json::Value::as_object_mut) {
                    let (state, routable) = match &lifecycle_state {
                        crate::runtime_server_control::RuntimeServerAgentSessionLifecycleState::Routable => ("live", true),
                        crate::runtime_server_control::RuntimeServerAgentSessionLifecycleState::Stopped => ("stopped", false),
                        crate::runtime_server_control::RuntimeServerAgentSessionLifecycleState::Achieved => ("achieved", false),
                        crate::runtime_server_control::RuntimeServerAgentSessionLifecycleState::Invalid => ("unregistered", false),
                    };
                    binding.insert("lifecycleState".to_owned(), state.into());
                    binding.insert("routable".to_owned(), routable.into());
                }
                Ok(crate::runtime_server_control::RuntimeServerAgentSessionStatus {
                    workspace_identity,
                    project_id: record.project_id().to_owned(),
                    root_session_id: record.root_session_id().to_owned(),
                    session_id: record.session_id().to_owned(),
                    name: record.name().to_owned(),
                    physical_generation: u64::try_from(record.physical_generation).unwrap_or(0),
                    lifecycle_state,
                    host_binding,
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
        Ok(*self.generation.borrow())
    }

    pub(crate) fn shared(
        &self,
    ) -> Arc<RwLock<Vec<crate::runtime_server_control::RuntimeServerAgentSessionStatus>>> {
        Arc::clone(&self.value)
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.generation.subscribe()
    }

    pub(crate) fn mark_current_published(&self) {
        let generation = *self.generation.borrow();
        self.published_generation.send_replace(generation);
    }
}
