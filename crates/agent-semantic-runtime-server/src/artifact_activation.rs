use std::future::Future;
use std::path::Path;
use std::path::PathBuf;

use agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent;
use agent_semantic_artifacts::runtime_artifact_activation::acknowledge_runtime_artifact_activation;
use agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event;
use agent_semantic_artifacts::runtime_artifact_activation::rollback_runtime_artifact_activation;
use agent_semantic_artifacts::runtime_artifact_activation::runtime_artifact_activation_socket_path;
use tokio::sync::oneshot;
use tokio::sync::watch;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactActivationReceipt {
    pub artifact_digest: agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    pub state: &'static str,
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeArtifactActivationDisposition {
    Committed,
    SuccessorRequired,
}

pub struct RuntimeArtifactActivationActor {
    endpoint: PathBuf,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<Result<(), String>>>,
    receipts: watch::Receiver<Option<RuntimeArtifactActivationReceipt>>,
}

impl RuntimeArtifactActivationActor {
    pub fn endpoint(&self) -> &Path {
        &self.endpoint
    }

    pub fn receipts(&self) -> watch::Receiver<Option<RuntimeArtifactActivationReceipt>> {
        self.receipts.clone()
    }

    pub async fn shutdown(mut self) -> Result<(), String> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        let task = self
            .task
            .take()
            .ok_or_else(|| "Runtime artifact activation actor task is absent".to_owned())?;
        task.await
            .map_err(|error| format!("Runtime artifact activation actor join failed: {error}"))?
    }
}

impl Drop for RuntimeArtifactActivationActor {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

pub async fn spawn_runtime_artifact_activation_actor<F, Fut>(
    state_home: PathBuf,
    activate: F,
) -> Result<RuntimeArtifactActivationActor, String>
where
    F: Fn(RuntimeArtifactActivationEvent) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<RuntimeArtifactActivationDisposition, String>> + Send + 'static,
{
    let endpoint = runtime_artifact_activation_socket_path(&state_home);
    let parent = endpoint.parent().ok_or_else(|| {
        format!(
            "Runtime activation endpoint has no parent: {}",
            endpoint.display()
        )
    })?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| format!("create Runtime activation endpoint directory: {error}"))?;
    if tokio::fs::try_exists(&endpoint)
        .await
        .map_err(|error| format!("inspect Runtime activation endpoint: {error}"))?
    {
        tokio::fs::remove_file(&endpoint)
            .await
            .map_err(|error| format!("remove stale Runtime activation endpoint: {error}"))?;
    }
    let socket = tokio::net::UnixDatagram::bind(&endpoint)
        .map_err(|error| format!("bind Runtime artifact activation actor: {error}"))?;
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
    let (receipt_tx, receipt_rx) = watch::channel(None);
    let task_endpoint = endpoint.clone();
    let task = tokio::spawn(async move {
        if let Some(event) = read_runtime_artifact_activation_event(&state_home).await? {
            match activate_and_acknowledge(&state_home, &activate, event.clone()).await {
                Ok(receipt) => {
                    let _ = receipt_tx.send(Some(receipt));
                }
                Err(error) => {
                    let _ = receipt_tx.send(Some(RuntimeArtifactActivationReceipt {
                        artifact_digest: event.artifact_digest,
                        state: "failed",
                        reason: Some(error),
                    }));
                }
            }
        }
        let mut buffer = vec![0_u8; 16 * 1024];
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                received = socket.recv(&mut buffer) => {
                    let received = received
                        .map_err(|error| format!("receive Runtime artifact activation event: {error}"))?;
                    let event: RuntimeArtifactActivationEvent = serde_json::from_slice(&buffer[..received])
                        .map_err(|error| format!("decode Runtime artifact activation datagram: {error}"))?;
                    match activate_and_acknowledge(&state_home, &activate, event.clone()).await {
                        Ok(receipt) => {
                            let _ = receipt_tx.send(Some(receipt));
                        }
                        Err(error) => {
                            let _ = receipt_tx.send(Some(RuntimeArtifactActivationReceipt {
                                artifact_digest: event.artifact_digest,
                                state: "failed",
                                reason: Some(error),
                            }));
                        }
                    }
                }
            }
        }
        let _ = tokio::fs::remove_file(&task_endpoint).await;
        Ok(())
    });
    Ok(RuntimeArtifactActivationActor {
        endpoint,
        shutdown: Some(shutdown_tx),
        task: Some(task),
        receipts: receipt_rx,
    })
}

pub async fn mount_runtime_daemon_artifact_activation<F, Fut>(
    state_home: PathBuf,
    activate: F,
) -> Result<RuntimeArtifactActivationActor, String>
where
    F: Fn(RuntimeArtifactActivationEvent) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<RuntimeArtifactActivationDisposition, String>> + Send + 'static,
{
    spawn_runtime_artifact_activation_actor(state_home, activate).await
}

async fn activate_and_acknowledge<F, Fut>(
    state_home: &Path,
    activate: &F,
    event: RuntimeArtifactActivationEvent,
) -> Result<RuntimeArtifactActivationReceipt, String>
where
    F: Fn(RuntimeArtifactActivationEvent) -> Fut,
    Fut: Future<Output = Result<RuntimeArtifactActivationDisposition, String>>,
{
    let artifact_digest = event.artifact_digest.clone();
    match activate(event.clone()).await {
        Ok(RuntimeArtifactActivationDisposition::Committed) => {
            acknowledge_runtime_artifact_activation(state_home, &artifact_digest).await?;
            Ok(RuntimeArtifactActivationReceipt {
                artifact_digest,
                state: "ready",
                reason: None,
            })
        }
        Ok(RuntimeArtifactActivationDisposition::SuccessorRequired) => {
            Ok(RuntimeArtifactActivationReceipt {
                artifact_digest,
                state: "successor-required",
                reason: None,
            })
        }
        Err(error) => {
            rollback_runtime_artifact_activation(state_home, &event).await?;
            Err(format!(
                "Runtime artifact activation failed: artifactDigest={artifact_digest} reasonKind=runtime-artifact-activation-failed activeRestoredFromHealthy=true error={error}"
            ))
        }
    }
}

#[cfg(all(test, unix))]
#[path = "../tests/unit/artifact_activation.rs"]
mod tests;
