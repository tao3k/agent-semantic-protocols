use std::future::Future;
use std::path::Path;
use std::path::PathBuf;

use agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent;
use agent_semantic_artifacts::runtime_artifact_activation::acknowledge_runtime_artifact_activation;
use agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event;
use agent_semantic_artifacts::runtime_artifact_activation::rollback_runtime_artifact_activation;
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
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<Result<(), String>>>,
    receipts: watch::Receiver<Option<RuntimeArtifactActivationReceipt>>,
}

impl RuntimeArtifactActivationActor {
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
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
    let (receipt_tx, receipt_rx) = watch::channel(None);
    let task = tokio::spawn(async move {
        let mut last_observed_event = None;
        if let Some(event) = read_runtime_artifact_activation_event(&state_home).await? {
            last_observed_event = Some((
                event.bundle_digest.clone(),
                event.artifact_digest.clone(),
                event.publication_nonce.clone(),
            ));
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
        let mut reconcile = tokio::time::interval(std::time::Duration::from_millis(50));
        reconcile.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                _ = reconcile.tick() => {
                    let Some(event) = read_runtime_artifact_activation_event(&state_home).await? else {
                        continue;
                    };
                    let event_identity = (
                        event.bundle_digest.clone(),
                        event.artifact_digest.clone(),
                        event.publication_nonce.clone(),
                    );
                    if last_observed_event.as_ref() == Some(&event_identity) {
                        continue;
                    }
                    last_observed_event = Some(event_identity);
                    let receipt = match activate_and_acknowledge(&state_home, &activate, event.clone()).await {
                        Ok(receipt) => receipt,
                        Err(error) => RuntimeArtifactActivationReceipt {
                            artifact_digest: event.artifact_digest,
                            state: "failed",
                            reason: Some(error),
                        },
                    };
                    if receipt_tx.send(Some(receipt)).is_err() {
                        break;
                    }
                }
            }
        }
        Ok(())
    });
    Ok(RuntimeArtifactActivationActor {
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
