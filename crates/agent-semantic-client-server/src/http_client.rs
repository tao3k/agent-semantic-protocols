use std::sync::atomic::{AtomicU64, Ordering};

use agent_semantic_client_protocol::{
    CLIENT_FRAME_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION, ClientFrame,
    ClientFrameBase, ClientInfo, ClientOutcome, ClientProtocolCatalog, ClientRequestId,
    ClientSessionId, ClientWorkspaceIdentity, SCHEMA_VERSION,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientHttpError {
    reason_kind: String,
    message: String,
}

fn decode_http_response(status: u16, body: &[u8]) -> Result<ClientFrame, String> {
    if !(200..300).contains(&status) {
        let error: ClientHttpError = serde_json::from_slice(body)
            .map_err(|decode| format!("decode ASP Client HTTP error status={status}: {decode}"))?;
        return Err(format!(
            "ASP Client HTTP error status={status} reasonKind={} message={}",
            error.reason_kind, error.message
        ));
    }
    serde_json::from_slice(body).map_err(|error| format!("decode client frame: {error}"))
}

/// Typed HTTP/JSON client for the Runtime-owned ASP Client Protocol endpoint.
pub struct AspClientProtocolHttpClient {
    connection: agent_semantic_http_json::persistent::HttpJsonConnection,
    base: ClientFrameBase,
    project_root: String,
    next_request_id: AtomicU64,
    catalog: Option<ClientProtocolCatalog>,
}

impl AspClientProtocolHttpClient {
    pub async fn connect(
        endpoint: impl Into<String>,
        workspace_identity: ClientWorkspaceIdentity,
        project_root: impl Into<String>,
    ) -> Result<Self, String> {
        let session_id = ClientSessionId::new(format!("asp-client-{}", std::process::id()))?;
        Ok(Self {
            connection: agent_semantic_http_json::persistent::HttpJsonConnection::connect(
                &endpoint.into(),
            )
            .await?,
            base: ClientFrameBase {
                schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
                schema_version: SCHEMA_VERSION.to_owned(),
                protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
                protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
                session_id,
                workspace_identity,
                trace_context: None,
            },
            project_root: project_root.into(),
            next_request_id: AtomicU64::new(1),
            catalog: None,
        })
    }

    pub fn next_request_id(&self) -> Result<ClientRequestId, String> {
        self.request_id("request")
    }

    fn request_id(&self, prefix: &str) -> Result<ClientRequestId, String> {
        ClientRequestId::new(format!(
            "{prefix}-{}",
            self.next_request_id.fetch_add(1, Ordering::Relaxed)
        ))
    }

    async fn post(&self, frame: &ClientFrame) -> Result<ClientFrame, String> {
        let body =
            serde_json::to_vec(frame).map_err(|error| format!("encode client frame: {error}"))?;
        let (status, body) = self
            .connection
            .post_json("/protocol/frame", body.into())
            .await?;
        decode_http_response(status, &body)
    }

    pub async fn initialize(&mut self) -> Result<ClientProtocolCatalog, String> {
        let frame = ClientFrame::Initialize {
            base: self.base.clone(),
            request_id: self.request_id("initialize")?,
            project_root: self.project_root.clone(),
            client_info: ClientInfo {
                name: "asp-client".to_owned(),
                version: "1".to_owned(),
            },
            capabilities: serde_json::json!({"requestCancellation": true}),
        };
        let response = self.post(&frame).await?;
        let ClientFrame::Response {
            outcome: ClientOutcome::Ready,
            catalog: Some(catalog),
            ..
        } = response
        else {
            return Err("ASP Client initialize did not return catalog".to_owned());
        };
        self.catalog = Some(catalog.clone());
        Ok(catalog)
    }

    pub async fn request(
        &self,
        method: &str,
        workspace_generation: &str,
        params: serde_json::Value,
    ) -> Result<ClientFrame, String> {
        let request_id = self.next_request_id()?;
        self.request_with_id(request_id, method, workspace_generation, params)
            .await
    }

    pub async fn request_with_id(
        &self,
        request_id: ClientRequestId,
        method: &str,
        workspace_generation: &str,
        params: serde_json::Value,
    ) -> Result<ClientFrame, String> {
        let catalog = self
            .catalog
            .as_ref()
            .ok_or_else(|| "ASP Client is not initialized".to_owned())?;
        if !catalog.methods.iter().any(|entry| entry.method == method) {
            return Err(format!("client method is not present in catalog: {method}"));
        }
        self.post(&ClientFrame::Request {
            base: self.base.clone(),
            request_id,
            catalog_generation: catalog.catalog_generation.clone(),
            workspace_generation: workspace_generation.to_owned(),
            method: method.to_owned(),
            params,
        })
        .await
    }

    pub async fn cancel(&self, request_id: ClientRequestId) -> Result<ClientFrame, String> {
        self.post(&ClientFrame::Cancel {
            base: self.base.clone(),
            request_id,
        })
        .await
    }

    pub async fn shutdown(self) -> Result<ClientFrame, String> {
        let response = self
            .post(&ClientFrame::Shutdown {
                base: self.base.clone(),
                request_id: self.request_id("shutdown")?,
            })
            .await?;
        self.connection.close().await?;
        Ok(response)
    }
}

#[cfg(test)]
#[path = "../tests/unit/http_client.rs"]
mod tests;
