use std::sync::Arc;

use crate::runtime_server::RuntimeServer;

impl RuntimeServer {
    #[must_use]
    pub fn with_agent_session_registry_owner(
        mut self,
        owner: Arc<crate::AgentSessionRegistry>,
    ) -> Self {
        let status = crate::runtime_server_agent_session_status::AgentSessionStatusHandle::new();
        self.status_memory.set_agent_sessions(status.shared());
        self.agent_session_status = Some(status);
        self.agent_session_registry_owner = Some(owner);
        self
    }
}
