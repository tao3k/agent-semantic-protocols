use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::net::UnixDatagram;

pub const READINESS_SOCKET_ENV: &str = "ASP_RUNTIME_READINESS_SINK";
pub const READINESS_TOKEN_ENV: &str = "ASP_RUNTIME_READINESS_TOKEN";
pub const READINESS_REQUEST_ID_ENV: &str = "ASP_RUNTIME_READINESS_REQUEST_ID";
pub const READINESS_MAX_PAYLOAD_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeServerReadinessState {
    Starting,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeServerReadinessReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub state: RuntimeServerReadinessState,
    pub request_id: String,
    pub readiness_token: String,
    pub process_id: u32,
    pub owner_epoch: u64,
    pub endpoint_binding_token: String,
    pub runtime_binary_identity: String,
    pub artifact_catalog_digest: String,
    pub transport_contract_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl RuntimeServerReadinessReceipt {
    pub const SCHEMA_ID: &'static str = "agent.semantic-protocols.runtime-server-readiness";
    pub const SCHEMA_VERSION: &'static str = "1";

    pub fn validate(&self, expected_request_id: &str, expected_token: &str) -> Result<(), String> {
        if self.schema_id != Self::SCHEMA_ID || self.schema_version != Self::SCHEMA_VERSION {
            return Err("runtime readiness schema identity mismatch".to_owned());
        }
        if self.request_id != expected_request_id || self.readiness_token != expected_token {
            return Err("runtime readiness request or token mismatch".to_owned());
        }
        if self.process_id == 0 || self.owner_epoch == 0 {
            return Err("runtime readiness owner identity is empty".to_owned());
        }
        for (name, value) in [
            ("endpointBindingToken", &self.endpoint_binding_token),
            ("runtimeBinaryIdentity", &self.runtime_binary_identity),
            ("artifactCatalogDigest", &self.artifact_catalog_digest),
            ("transportContractDigest", &self.transport_contract_digest),
        ] {
            if value.is_empty() {
                return Err(format!("runtime readiness {name} is empty"));
            }
        }
        if matches!(
            self.state,
            RuntimeServerReadinessState::Ready
                | RuntimeServerReadinessState::Failed
                | RuntimeServerReadinessState::Cancelled
        ) && self.reason_kind.is_none()
        {
            return Err("runtime readiness terminal state lacks reasonKind".to_owned());
        }
        Ok(())
    }
}

pub struct RuntimeServerReadinessListener {
    socket: UnixDatagram,
    path: PathBuf,
    request_id: String,
    readiness_token: String,
}

impl RuntimeServerReadinessListener {
    #[cfg(test)]
    async fn bind(
        state_home: &std::path::Path,
        request_id: impl Into<String>,
        readiness_token: impl Into<String>,
    ) -> Result<Self, String> {
        let root = RuntimeServerReadinessRoot::new(state_home)?;
        Self::bind_root(&root, request_id, readiness_token).await
    }

    pub fn socket_path(&self) -> &Path {
        &self.path
    }

    pub fn launch_environment(&self) -> [(String, String); 3] {
        [
            (
                READINESS_SOCKET_ENV.to_owned(),
                self.path.display().to_string(),
            ),
            (READINESS_TOKEN_ENV.to_owned(), self.readiness_token.clone()),
            (READINESS_REQUEST_ID_ENV.to_owned(), self.request_id.clone()),
        ]
    }

    pub async fn receive(
        &self,
        expected_process_id: u32,
    ) -> Result<RuntimeServerReadinessReceipt, String> {
        let mut buffer = vec![0_u8; READINESS_MAX_PAYLOAD_BYTES + 1];
        loop {
            let size = self
                .socket
                .recv(&mut buffer)
                .await
                .map_err(|e| e.to_string())?;
            if size > READINESS_MAX_PAYLOAD_BYTES {
                continue;
            }
            let receipt: RuntimeServerReadinessReceipt =
                match serde_json::from_slice(&buffer[..size]) {
                    Ok(receipt) => receipt,
                    Err(_) => continue,
                };
            if receipt.process_id != expected_process_id
                || receipt
                    .validate(&self.request_id, &self.readiness_token)
                    .is_err()
            {
                continue;
            }
            return Ok(receipt);
        }
    }

    pub async fn cleanup(self) -> Result<(), String> {
        drop(self.socket);
        match tokio::fs::remove_file(&self.path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

pub struct RuntimeServerReadinessPublisher {
    socket: UnixDatagram,
    request_id: String,
    readiness_token: String,
}

impl RuntimeServerReadinessPublisher {
    pub async fn from_environment() -> Result<Self, String> {
        let path = std::env::var(READINESS_SOCKET_ENV)
            .map_err(|_| "readiness socket env is missing".to_owned())?;
        let request_id = std::env::var(READINESS_REQUEST_ID_ENV)
            .map_err(|_| "readiness request env is missing".to_owned())?;
        let readiness_token = std::env::var(READINESS_TOKEN_ENV)
            .map_err(|_| "readiness token env is missing".to_owned())?;
        Self::connect(Path::new(&path), request_id, readiness_token).await
    }

    pub async fn connect(
        path: &Path,
        request_id: impl Into<String>,
        readiness_token: impl Into<String>,
    ) -> Result<Self, String> {
        let socket = UnixDatagram::unbound().map_err(|e| e.to_string())?;
        socket.connect(path).map_err(|e| e.to_string())?;
        Ok(Self {
            socket,
            request_id: request_id.into(),
            readiness_token: readiness_token.into(),
        })
    }

    pub async fn publish(&self, mut receipt: RuntimeServerReadinessReceipt) -> Result<(), String> {
        receipt.request_id = self.request_id.clone();
        receipt.readiness_token = self.readiness_token.clone();
        receipt.validate(&self.request_id, &self.readiness_token)?;
        let payload = serde_json::to_vec(&receipt).map_err(|e| e.to_string())?;
        if payload.len() > READINESS_MAX_PAYLOAD_BYTES {
            return Err("runtime readiness receipt exceeds payload limit".to_owned());
        }
        self.socket
            .send(&payload)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(state: RuntimeServerReadinessState, token: &str) -> RuntimeServerReadinessReceipt {
        RuntimeServerReadinessReceipt {
            schema_id: RuntimeServerReadinessReceipt::SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            state,
            request_id: "request".to_owned(),
            readiness_token: token.to_owned(),
            process_id: 42,
            owner_epoch: 7,
            endpoint_binding_token: "binding".to_owned(),
            runtime_binary_identity: "binary".to_owned(),
            artifact_catalog_digest: "catalog".to_owned(),
            transport_contract_digest: "transport".to_owned(),
            reason_kind: Some("test".to_owned()),
            error: None,
        }
    }

    #[tokio::test]
    async fn readiness_ready_roundtrip_and_cleanup() {
        let home = tempfile::Builder::new()
            .prefix("asp-rdy-")
            .tempdir_in("/tmp")
            .unwrap();
        let listener = RuntimeServerReadinessListener::bind(home.path(), "request", "token")
            .await
            .unwrap();
        let publisher =
            RuntimeServerReadinessPublisher::connect(listener.socket_path(), "request", "token")
                .await
                .unwrap();
        publisher
            .publish(receipt(RuntimeServerReadinessState::Ready, "wrong"))
            .await
            .unwrap();
        let received = listener.receive(42).await.unwrap();
        assert_eq!(received.state, RuntimeServerReadinessState::Ready);
        let path = listener.socket_path().to_owned();
        listener.cleanup().await.unwrap();
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn stale_token_and_malformed_payload_are_ignored() {
        let home = tempfile::Builder::new()
            .prefix("asp-rdy-")
            .tempdir_in("/tmp")
            .unwrap();
        let listener = RuntimeServerReadinessListener::bind(home.path(), "request", "token")
            .await
            .unwrap();
        let stale =
            RuntimeServerReadinessPublisher::connect(listener.socket_path(), "request", "stale")
                .await
                .unwrap();
        let good =
            RuntimeServerReadinessPublisher::connect(listener.socket_path(), "request", "token")
                .await
                .unwrap();
        stale
            .publish(receipt(RuntimeServerReadinessState::Ready, "stale"))
            .await
            .unwrap();
        good.publish(receipt(RuntimeServerReadinessState::Ready, "token"))
            .await
            .unwrap();
        assert_eq!(listener.receive(42).await.unwrap().readiness_token, "token");
        listener.cleanup().await.unwrap();
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeServerReadinessRoot(std::path::PathBuf);

impl RuntimeServerReadinessRoot {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Result<Self, String> {
        let path = path.into();
        if !path.is_absolute() {
            return Err(format!(
                "Runtime readiness root must be absolute: {}",
                path.display()
            ));
        }
        Ok(Self(path))
    }

    pub fn as_path(&self) -> &std::path::Path {
        &self.0
    }

    pub fn endpoint(
        &self,
        request_id: &str,
        readiness_token: &str,
    ) -> RuntimeServerReadinessEndpoint {
        let mut identity = blake3::Hasher::new();
        identity.update(b"agent.semantic-protocols.runtime-server-readiness-endpoint.v1\0");
        identity.update(request_id.as_bytes());
        identity.update(b"\0");
        identity.update(readiness_token.as_bytes());
        let identity = identity.finalize().to_hex();
        RuntimeServerReadinessEndpoint(self.0.join("r").join(format!("{}.sock", &identity[..32])))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeServerReadinessEndpoint(std::path::PathBuf);

impl RuntimeServerReadinessEndpoint {
    pub fn as_path(&self) -> &std::path::Path {
        &self.0
    }
}

impl RuntimeServerReadinessListener {
    pub async fn bind_root(
        root: &RuntimeServerReadinessRoot,
        request_id: impl Into<String>,
        readiness_token: impl Into<String>,
    ) -> Result<Self, String> {
        let request_id = request_id.into();
        let readiness_token = readiness_token.into();
        let endpoint = root.endpoint(&request_id, &readiness_token);
        let parent = endpoint
            .as_path()
            .parent()
            .ok_or_else(|| "Runtime readiness endpoint has no parent".to_owned())?;
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            format!(
                "create Runtime readiness endpoint directory {}: {error}",
                parent.display()
            )
        })?;
        match tokio::fs::remove_file(endpoint.as_path()).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "remove stale Runtime readiness endpoint {}: {error}",
                    endpoint.as_path().display()
                ));
            }
        }
        let socket = tokio::net::UnixDatagram::bind(endpoint.as_path()).map_err(|error| {
            format!(
                "bind Runtime readiness endpoint {}: {error}",
                endpoint.as_path().display()
            )
        })?;
        Ok(Self {
            socket,
            path: endpoint.0,
            request_id,
            readiness_token,
        })
    }

    pub fn endpoint(&self) -> RuntimeServerReadinessEndpoint {
        RuntimeServerReadinessEndpoint(self.path.clone())
    }
}
pub async fn await_monitored_candidate_readiness(
    listener: &RuntimeServerReadinessListener,
    process: &mut agent_semantic_runtime::runtime_process_lifecycle::RuntimeProcessLaunchHandle,
) -> Result<RuntimeServerReadinessReceipt, String> {
    let process_id = process.process_id();
    tokio::select! {
        readiness = listener.receive(process_id) => readiness,
        exit = process.wait() => {
            let exit = exit?;
            Err(format!(
                "Runtime Server candidate exited before readiness: reasonKind=runtime-server-candidate-exited-before-readiness processId={process_id} exitStatus={exit}"
            ))
        }
    }
}

pub async fn launch_candidate_and_await_readiness(
    listener: &RuntimeServerReadinessListener,
    spec: agent_semantic_runtime::runtime_process_lifecycle::RuntimeProcessLaunchSpec,
) -> Result<
    (
        RuntimeServerReadinessReceipt,
        agent_semantic_runtime::runtime_process_lifecycle::RuntimeProcessLaunchHandle,
    ),
    String,
> {
    let mut process =
        agent_semantic_runtime::runtime_process_lifecycle::launch_monitored(spec)
            .await
            .map_err(|error| {
                format!(
                    "Runtime Server candidate spawn failed: reasonKind=runtime-server-candidate-spawn-failed error={error}"
                )
            })?;
    let readiness = await_monitored_candidate_readiness(listener, &mut process).await?;
    Ok((readiness, process))
}
