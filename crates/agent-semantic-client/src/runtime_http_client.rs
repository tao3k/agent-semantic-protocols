use std::path::PathBuf;

use agent_semantic_client_protocol::{ClientFrame, ClientWorkspaceIdentity};
use agent_semantic_client_server::AspClientProtocolHttpClient;

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

    /// Dispatch one typed method to the resident ASP Server.
    pub async fn dispatch(
        &self,
        language_id: &str,
        route: &str,
        params: serde_json::Value,
    ) -> Result<ClientFrame, String> {
        let endpoint = agent_semantic_client_db::read_runtime_server_endpoint(&self.state_home)?
            .ok_or_else(|| "ASP Server endpoint is unavailable".to_owned())?;
        endpoint.validate()?;
        let workspace_identity =
            agent_semantic_client_db::AgentSessionRegistry::workspace_id(&self.project_root)?;
        let client = AspClientProtocolHttpClient::connect(
            endpoint.client_http_endpoint,
            ClientWorkspaceIdentity::new(workspace_identity)?,
            self.project_root.display().to_string(),
        )
        .await?;
        client
            .dispatch(&format!("{language_id}.{route}"), params)
            .await
    }
}
