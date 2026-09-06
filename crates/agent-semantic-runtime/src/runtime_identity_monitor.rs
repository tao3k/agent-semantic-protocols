//! Runtime-owned identity observation state machine and Tokio monitor task.

use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Serialize;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinHandle;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MonitorAction {
    Noop,
    BeginDrain,
    SpawnLatest,
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeIdentityMonitor {
    observed: Option<String>,
    draining: bool,
    latest: Option<String>,
}

impl RuntimeIdentityMonitor {
    pub fn observe(&mut self, identity: impl Into<String>) -> MonitorAction {
        let identity = identity.into();
        if self.observed.as_deref() == Some(identity.as_str()) && !self.draining {
            return MonitorAction::Noop;
        }
        self.latest = Some(identity);
        if self.draining {
            MonitorAction::Noop
        } else {
            self.draining = true;
            MonitorAction::BeginDrain
        }
    }

    pub fn drain_completed(&mut self) -> MonitorAction {
        self.draining = false;
        if self.latest != self.observed {
            self.observed = self.latest.clone();
            MonitorAction::SpawnLatest
        } else {
            MonitorAction::Noop
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeIdentityChanged {
    pub previous_identity: String,
    pub observed_identity: String,
}

pub struct RuntimeIdentityMonitorHandle {
    events: mpsc::Receiver<RuntimeIdentityChanged>,
    cancel: watch::Sender<bool>,
    task: JoinHandle<()>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResidentActivationIdentity {
    pub(crate) activation_generation: u64,
    pub(crate) artifact_digest:
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    pub(crate) publication_nonce: String,
    pub(crate) owner_epoch: u64,
}

impl RuntimeIdentityMonitorHandle {
    pub async fn next_event(&mut self) -> Option<RuntimeIdentityChanged> {
        self.events.recv().await
    }

    pub async fn shutdown(self) {
        let _ = self.cancel.send(true);
        let _ = self.task.await;
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeIdentityMonitorReceipt<'a> {
    schema_id: &'static str,
    schema_version: &'static str,
    phase: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    running_identity: Option<&'a ResidentActivationIdentity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observed_identity: Option<&'a ResidentActivationIdentity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observation_error: Option<&'a str>,
    owner_epoch: u64,
    process_id: u32,
    updated_at_millis: u128,
    heartbeat: bool,
}

pub fn spawn_runtime_identity_monitor(
    state_home: PathBuf,
    owner_epoch: u64,
    activation_generation: u64,
    publication_nonce: String,
    artifact_digest: agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    artifact_mode: &str,
) -> RuntimeIdentityMonitorHandle {
    spawn_runtime_identity_monitor_with_intervals(
        state_home,
        owner_epoch,
        Some(ResidentActivationIdentity {
            activation_generation,
            artifact_digest,
            publication_nonce,
            owner_epoch,
        }),
        runtime_identity_poll_interval(artifact_mode),
        Duration::from_secs(30),
    )
}

pub(crate) fn runtime_identity_poll_interval(artifact_mode: &str) -> Duration {
    if artifact_mode == "dev" {
        Duration::from_millis(50)
    } else {
        Duration::from_secs(5)
    }
}

pub(crate) fn spawn_runtime_identity_monitor_with_intervals(
    state_home: PathBuf,
    owner_epoch: u64,
    running_identity: Option<ResidentActivationIdentity>,
    poll_interval: Duration,
    heartbeat_interval: Duration,
) -> RuntimeIdentityMonitorHandle {
    let (event_tx, event_rx) = mpsc::channel(1);
    let (cancel_tx, mut cancel_rx) = watch::channel(false);
    let task = tokio::spawn(async move {
        let _ = write_monitor_receipt(
            &state_home,
            owner_epoch,
            "starting",
            None,
            None,
            None,
            false,
        )
        .await;
        let mut tick =
            tokio::time::interval_at(tokio::time::Instant::now() + poll_interval, poll_interval);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut last_identity = running_identity;
        let mut last_heartbeat = tokio::time::Instant::now();
        let mut last_applied_failure = None;
        loop {
            tokio::select! {
                changed = cancel_rx.changed() => {
                    if changed.is_err() || *cancel_rx.borrow() {
                        break;
                    }
                }
                _ = tick.tick() => {
                    let applied = match agent_semantic_artifacts::runtime_artifact_activation::
                        read_applied_runtime_artifact_activation_event(&state_home).await {
                        Ok(Some(applied)) => {
                            last_applied_failure = None;
                            applied
                        }
                        Ok(None) => {
                            let failure = "applied-activation-absent".to_owned();
                            if last_applied_failure.as_ref() != Some(&failure) {
                                let _ = write_monitor_receipt(
                                    &state_home,
                                    owner_epoch,
                                    "applied-authority-unavailable",
                                    last_identity.as_ref(),
                                    None,
                                    Some(&failure),
                                    false,
                                ).await;
                                last_applied_failure = Some(failure);
                            }
                            continue;
                        }
                        Err(error) => {
                            let failure = format!("applied-activation-invalid:{error}");
                            if last_applied_failure.as_ref() != Some(&failure) {
                                let _ = write_monitor_receipt(
                                    &state_home,
                                    owner_epoch,
                                    "applied-authority-unavailable",
                                    last_identity.as_ref(),
                                    None,
                                    Some(&failure),
                                    false,
                                ).await;
                                last_applied_failure = Some(failure);
                            }
                            continue;
                        }
                    };
                let identity = ResidentActivationIdentity {
                    activation_generation: applied.activation_generation,
                    artifact_digest: applied.artifact_digest,
                        publication_nonce: applied.publication_nonce,
                        owner_epoch,
                    };
                    let identity_label = runtime_identity_label(&identity);
                    match last_identity.as_ref() {
                        None => {
                            let _ = write_monitor_receipt(
                                &state_home,
                                owner_epoch,
                                "watching",
                                Some(&identity),
                                Some(&identity),
                                None,
                                true,
                            ).await;
                            last_heartbeat = tokio::time::Instant::now();
                            last_identity = Some(identity);
                        }
                    Some(previous)
                        if identity.activation_generation < previous.activation_generation =>
                    {
                        // The applied receipt is an older Healthy publication.
                        // Sequence is used only as a fence; it never makes two
                        // different content identities equal.
                        continue;
                    }
                    Some(previous) if previous != &identity => {
                            let previous_label = runtime_identity_label(previous);
                            let event = RuntimeIdentityChanged {
                                previous_identity: previous_label.clone(),
                                observed_identity: identity_label.clone(),
                            };
                            let _ = write_monitor_receipt(
                                &state_home,
                                owner_epoch,
                                "observed",
                                Some(previous),
                                Some(&identity),
                                None,
                                false,
                            ).await;
                            let _ = event_tx.send(event).await;
                            break;
                        }
                        Some(previous) if last_heartbeat.elapsed() >= heartbeat_interval => {
                            let _ = write_monitor_receipt(
                                &state_home,
                                owner_epoch,
                                "watching",
                                Some(previous),
                                Some(previous),
                                None,
                                true,
                            ).await;
                            last_heartbeat = tokio::time::Instant::now();
                        }
                        Some(_) => {}
                    }
                }
            }
        }
    });
    RuntimeIdentityMonitorHandle {
        events: event_rx,
        cancel: cancel_tx,
        task,
    }
}

fn runtime_identity_label(identity: &ResidentActivationIdentity) -> String {
    format!(
        "digest:{} publicationNonce:{} ownerEpoch:{}",
        identity.artifact_digest, identity.publication_nonce, identity.owner_epoch
    )
}

async fn write_monitor_receipt(
    state_home: &Path,
    owner_epoch: u64,
    phase: &str,
    running_identity: Option<&ResidentActivationIdentity>,
    observed_identity: Option<&ResidentActivationIdentity>,
    observation_error: Option<&str>,
    heartbeat: bool,
) -> Result<(), String> {
    let path = agent_semantic_artifacts::StateHomeLayout::new(state_home)
        .runtime_state()
        .serving()
        .identity_monitor_receipt();
    let parent = path
        .parent()
        .ok_or_else(|| "Runtime identity monitor receipt has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| error.to_string())?;
    let staged = path.with_extension(format!("stage-{owner_epoch}"));
    let receipt = RuntimeIdentityMonitorReceipt {
        schema_id: "agent.semantic-protocols.runtime-resident-identity-monitor-receipt",
        schema_version: "1",
        phase,
        running_identity,
        observed_identity,
        observation_error,
        owner_epoch,
        process_id: crate::runtime_process_lifecycle::current_process_id(),
        updated_at_millis: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default(),
        heartbeat,
    };
    tokio::fs::write(
        &staged,
        serde_json::to_vec(&receipt).map_err(|error| error.to_string())?,
    )
    .await
    .map_err(|error| error.to_string())?;
    tokio::fs::rename(&staged, &path)
        .await
        .map_err(|error| error.to_string())
}
