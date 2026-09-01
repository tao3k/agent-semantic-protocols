//! Runtime Server identity successor handoff and its ordered test contract.

use agent_semantic_client_db::runtime_server_control::cleanup_endpoint;

pub(super) struct RuntimeIdentityHandoffCoordinator<'a> {
    state_home: &'a std::path::Path,
    endpoint: &'a agent_semantic_client_db::runtime_server_control::RuntimeServerEndpoint,
}

impl<'a> RuntimeIdentityHandoffCoordinator<'a> {
    pub(super) fn new(
        state_home: &'a std::path::Path,
        endpoint: &'a agent_semantic_client_db::runtime_server_control::RuntimeServerEndpoint,
    ) -> Self {
        Self {
            state_home,
            endpoint,
        }
    }

    pub(super) async fn retire(&self) -> Result<(), String> {
        agent_semantic_client_db::runtime_server_supervisor::retire_runtime_server_owner_for_handoff(
            self.state_home,
            self.endpoint,
        )
        .await
    }

    pub(super) async fn cleanup(&self) -> Result<(), String> {
        cleanup_endpoint(self.state_home, self.endpoint).await
    }
}
