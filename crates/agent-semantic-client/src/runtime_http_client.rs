//! Persistent Runtime-owned ASP Client Protocol session.

use agent_semantic_client_protocol::{ClientFrame, ClientProtocolCatalog, ClientWorkspaceIdentity};
use agent_semantic_client_server::AspClientProtocolHttpClient;
use std::path::PathBuf;

/// Runtime endpoint/session bootstrap owner for the ASP Client Protocol.
pub struct RuntimeHttpClient {
    state_home: PathBuf,
    project_root: PathBuf,
}

/// Persistent initialized ASP Client Protocol session.
pub struct RuntimeHttpSession {
    inner: AspClientProtocolHttpClient,
    catalog: ClientProtocolCatalog,
}

impl RuntimeHttpClient {
    pub fn new(state_home: impl Into<PathBuf>, project_root: impl Into<PathBuf>) -> Self {
        Self {
            state_home: state_home.into(),
            project_root: project_root.into(),
        }
    }

    pub async fn open_session(&self) -> Result<RuntimeHttpSession, String> {
        let endpoint = agent_semantic_client_db::read_runtime_server_endpoint(&self.state_home)?
            .ok_or_else(|| "Runtime Server endpoint is unavailable".to_owned())?;
        endpoint.validate()?;
        let workspace_identity =
            agent_semantic_client_db::AgentSessionRegistry::workspace_id(&self.project_root)?;
        let mut inner = AspClientProtocolHttpClient::connect(
            endpoint.client_http_endpoint,
            ClientWorkspaceIdentity::new(workspace_identity)?,
            self.project_root.display().to_string(),
        )
        .await?;
        let catalog = inner.initialize().await?;
        Ok(RuntimeHttpSession { inner, catalog })
    }
}

impl RuntimeHttpSession {
    /// Send one catalog-validated request over the persistent session.
    pub async fn request_route(
        &self,
        language_id: &str,
        route: &str,
        params: serde_json::Value,
    ) -> Result<ClientFrame, String> {
        let method = format!("{language_id}.{route}");
        if !self
            .catalog
            .methods
            .iter()
            .any(|entry| entry.method == method)
        {
            return Err(format!("Runtime catalog method is unavailable: {method}"));
        }
        self.inner
            .request(&method, &self.catalog.workspace_generation, params)
            .await
    }

    pub async fn shutdown(self) -> Result<ClientFrame, String> {
        self.inner.shutdown().await
    }
}
