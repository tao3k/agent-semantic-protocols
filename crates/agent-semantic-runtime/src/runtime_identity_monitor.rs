//! Runtime-owned identity observation state machine and Tokio monitor task.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::{mpsc, watch};
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
    schema_version: &'static str,
    phase: &'a str,
    running_identity: &'a str,
    observed_identity: &'a str,
    owner_epoch: u64,
    process_id: u32,
    updated_at_millis: u128,
    heartbeat: bool,
}

pub fn spawn_runtime_identity_monitor(
    state_home: PathBuf,
    artifact_kind: String,
    owner_epoch: u64,
) -> RuntimeIdentityMonitorHandle {
    spawn_runtime_identity_monitor_with_intervals(
        state_home,
        artifact_kind,
        owner_epoch,
        Duration::from_secs(5),
        Duration::from_secs(30),
    )
}

pub(crate) fn spawn_runtime_identity_monitor_with_intervals(
    state_home: PathBuf,
    artifact_kind: String,
    owner_epoch: u64,
    poll_interval: Duration,
    heartbeat_interval: Duration,
) -> RuntimeIdentityMonitorHandle {
    let (event_tx, event_rx) = mpsc::channel(1);
    let (cancel_tx, mut cancel_rx) = watch::channel(false);
    let task = tokio::spawn(async move {
        let mut tick = tokio::time::interval(poll_interval);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut last_identity: Option<String> = None;
        let mut last_heartbeat = tokio::time::Instant::now();
        loop {
            tokio::select! {
                changed = cancel_rx.changed() => {
                    if changed.is_err() || *cancel_rx.borrow() {
                        break;
                    }
                }
                _ = tick.tick() => {
                    let Ok(receipt) = crate::runtime_artifact_identity::read_runtime_artifact_identity(
                        &state_home,
                        &artifact_kind,
                    ).await else {
                        continue;
                    };
                    let identity = format!(
                        "{}:{}:{}",
                        receipt.identity_kind(),
                        receipt.identity_value(),
                        receipt.identity_algorithm(),
                    );
                    match last_identity.as_deref() {
                        None => {
                            let _ = write_monitor_receipt(
                                &state_home,
                                owner_epoch,
                                "watching",
                                &identity,
                                &identity,
                                true,
                            ).await;
                            last_heartbeat = tokio::time::Instant::now();
                            last_identity = Some(identity);
                        }
                        Some(previous) if previous != identity => {
                            let event = RuntimeIdentityChanged {
                                previous_identity: previous.to_owned(),
                                observed_identity: identity.clone(),
                            };
                            let _ = write_monitor_receipt(
                                &state_home,
                                owner_epoch,
                                "observed",
                                previous,
                                &identity,
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
                                previous,
                                previous,
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

async fn write_monitor_receipt(
    state_home: &Path,
    owner_epoch: u64,
    phase: &str,
    running_identity: &str,
    observed_identity: &str,
    heartbeat: bool,
) -> Result<(), String> {
    let path = state_home.join("runtime/server/monitor-state.json");
    let parent = path
        .parent()
        .ok_or_else(|| "Runtime identity monitor receipt has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| error.to_string())?;
    let staged = path.with_extension(format!("stage-{owner_epoch}"));
    let receipt = RuntimeIdentityMonitorReceipt {
        schema_version: "1",
        phase,
        running_identity,
        observed_identity,
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
