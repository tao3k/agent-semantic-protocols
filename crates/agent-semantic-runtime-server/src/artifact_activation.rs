use std::future::Future;
use std::path::{Path, PathBuf};

use agent_semantic_artifacts::runtime_artifact_publication::{
    RuntimeArtifactActivationEvent, acknowledge_runtime_artifact_activation,
    read_runtime_artifact_activation_event, runtime_artifact_activation_socket_path,
};
use tokio::sync::{oneshot, watch};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactActivationReceipt {
    pub artifact_digest: agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    pub state: &'static str,
    pub reason: Option<String>,
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
    Fut: Future<Output = Result<(), String>> + Send + 'static,
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
            let receipt = activate_and_acknowledge(&state_home, &activate, event).await?;
            let _ = receipt_tx.send(Some(receipt));
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
                    let receipt = activate_and_acknowledge(&state_home, &activate, event).await?;
                    let _ = receipt_tx.send(Some(receipt));
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
    Fut: Future<Output = Result<(), String>> + Send + 'static,
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
    Fut: Future<Output = Result<(), String>>,
{
    let artifact_digest = event.artifact_digest.clone();
    match activate(event).await {
        Ok(()) => {
            acknowledge_runtime_artifact_activation(state_home, &artifact_digest).await?;
            Ok(RuntimeArtifactActivationReceipt {
                artifact_digest,
                state: "ready",
                reason: None,
            })
        }
        Err(error) => Err(format!(
            "Runtime artifact activation failed: artifactDigest={artifact_digest} reasonKind=runtime-artifact-activation-failed error={error}"
        )),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[tokio::test]
    async fn daemon_composition_consumes_startup_receipt_and_online_datagram() {
        let temporary = tempfile::tempdir().expect("temporary state");
        let state_home = temporary.path().join("state");
        let source = temporary.path().join("asp");
        let target = temporary.path().join("bin/asp");
        tokio::fs::write(&source, b"#!/bin/sh\nexit 77\n")
            .await
            .expect("write artifact");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
            .expect("artifact permissions");

        let startup_publication =
            publish_runtime_artifact(&state_home, &source, &target, "dev", None)
                .await
                .expect("publication succeeds without Runtime actor");
        let actor =
            mount_runtime_daemon_artifact_activation(state_home.clone(), |_event| async { Ok(()) })
                .await
                .expect("mount daemon activation composition");

        let mut receipts = actor.receipts();
        receipts
            .changed()
            .await
            .expect("startup activation receipt");
        let startup_terminal = receipts.borrow().clone().expect("typed startup receipt");
        assert_eq!(startup_terminal.state, "ready");
        assert_eq!(
            startup_terminal.artifact_digest,
            startup_publication.artifact_digest
        );

        tokio::fs::write(&source, b"#!/bin/sh\nexit 78\n")
            .await
            .expect("update artifact");
        let online_publication = publish_runtime_artifact(
            &state_home,
            &source,
            &target,
            "dev",
            Some(&startup_publication.artifact_digest),
        )
        .await
        .expect("online publication");
        receipts.changed().await.expect("online activation receipt");
        let online_terminal = receipts.borrow().clone().expect("typed online receipt");
        assert_eq!(online_terminal.state, "ready");
        assert_eq!(
            online_terminal.artifact_digest,
            online_publication.artifact_digest
        );
        assert!(
            read_runtime_artifact_activation_event(&state_home)
                .await
                .unwrap()
                .is_none()
        );
        actor.shutdown().await.expect("shutdown activation actor");
    }
}
