use std::sync::Arc;

use crate::runtime_server::RuntimeServer;

impl RuntimeServer {
    #[must_use]
    pub fn with_agent_session_registry_owner(
        mut self,
        owner: Arc<crate::AgentSessionRegistry>,
    ) -> Self {
        self.agent_session_registry_owner = Some(owner);
        self
    }
}
