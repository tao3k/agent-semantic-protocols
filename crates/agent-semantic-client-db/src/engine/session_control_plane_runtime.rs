// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    Arc, Path, SessionControlPlaneAgentRegistration, SessionControlPlaneDelegationProposal,
    SessionControlPlaneRuntimeMetricsSnapshot, SessionControlPlaneSnapshot,
    SessionControlPlaneTransactionOwner, SessionControlPlaneTransactionReceipt,
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};

#[derive(Debug, Clone)]
pub struct SessionControlPlaneRuntime {
    inner: Arc<SessionControlPlaneRuntimeInner>,
}

#[derive(Clone, Debug, Default)]
pub struct SessionControlPlaneRuntimeRegistry {
    #[expect(
        clippy::type_complexity,
        reason = "the registry explicitly binds project paths to single-flight runtime cells"
    )]
    entries: Arc<
        tokio::sync::RwLock<
            BTreeMap<
                PathBuf,
                Arc<tokio::sync::OnceCell<Result<Arc<SessionControlPlaneRuntime>, String>>>,
            >,
        >,
    >,
}

#[derive(Debug)]
struct SessionControlPlaneRuntimeInner {
    data_plane: SessionControlPlaneTransactionOwner,
    transition_sender: tokio::sync::mpsc::Sender<SessionControlPlaneTransition>,
    shutdown_sender: tokio::sync::watch::Sender<bool>,
    task: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    stream_metrics: Arc<SessionControlPlaneStreamMetrics>,
}

#[derive(Debug, Default)]
struct SessionControlPlaneStreamMetrics {
    queue_depth: AtomicU64,
    queue_high_watermark: AtomicU64,
    enqueued_transitions: AtomicU64,
    completed_transitions: AtomicU64,
    drained_transitions: AtomicU64,
    cancelled_transitions: AtomicU64,
    actor_starts: AtomicU64,
    actor_stops: AtomicU64,
}

#[derive(Debug)]
enum SessionControlPlaneTransition {
    RegisterAgent {
        registration: SessionControlPlaneAgentRegistration,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    AdmitDelegation {
        proposal: SessionControlPlaneDelegationProposal,
        reply: tokio::sync::oneshot::Sender<Result<SessionControlPlaneTransactionReceipt, String>>,
    },
}

impl SessionControlPlaneRuntime {
    pub async fn start_in_client_dir(client_dir: impl AsRef<Path>) -> Result<Self, String> {
        let data_plane =
            SessionControlPlaneTransactionOwner::open_in_client_dir(client_dir).await?;
        let queue_capacity = std::thread::available_parallelism()
            .map(|parallelism| parallelism.get())
            .unwrap_or(1)
            .saturating_mul(32)
            .clamp(256, 4096);
        let (transition_sender, transition_receiver) = tokio::sync::mpsc::channel(queue_capacity);
        let (shutdown_sender, shutdown_receiver) = tokio::sync::watch::channel(false);
        let actor_data_plane = data_plane.clone();
        let stream_metrics = Arc::new(SessionControlPlaneStreamMetrics::default());
        let task = tokio::spawn(run_session_control_plane_transition_stream(
            actor_data_plane,
            ReceiverStream::new(transition_receiver),
            shutdown_receiver,
            Arc::clone(&stream_metrics),
        ));
        Ok(Self {
            inner: Arc::new(SessionControlPlaneRuntimeInner {
                data_plane,
                transition_sender,
                shutdown_sender,
                task: tokio::sync::Mutex::new(Some(task)),
                stream_metrics,
            }),
        })
    }

    pub async fn register_agent(
        &self,
        registration: &SessionControlPlaneAgentRegistration,
    ) -> Result<(), String> {
        let (reply, result) = tokio::sync::oneshot::channel();
        let permit = self
            .inner
            .transition_sender
            .reserve()
            .await
            .map_err(|_| "session control-plane transition stream is closed".to_owned())?;
        self.record_transition_enqueued();
        permit.send(SessionControlPlaneTransition::RegisterAgent {
            registration: registration.clone(),
            reply,
        });
        result
            .await
            .map_err(|_| "session control-plane register receipt was dropped".to_owned())?
    }

    pub async fn admit_delegation(
        &self,
        proposal: &SessionControlPlaneDelegationProposal,
    ) -> Result<SessionControlPlaneTransactionReceipt, String> {
        if let Some(receipt) = self
            .inner
            .data_plane
            .resident_committed_receipt(proposal)
            .await
        {
            return Ok(receipt);
        }
        let (reply, result) = tokio::sync::oneshot::channel();
        let permit = self
            .inner
            .transition_sender
            .reserve()
            .await
            .map_err(|_| "session control-plane transition stream is closed".to_owned())?;
        self.record_transition_enqueued();
        permit.send(SessionControlPlaneTransition::AdmitDelegation {
            proposal: proposal.clone(),
            reply,
        });
        result
            .await
            .map_err(|_| "session control-plane admission receipt was dropped".to_owned())?
    }

    fn record_transition_enqueued(&self) {
        let depth = self
            .inner
            .stream_metrics
            .queue_depth
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        self.inner
            .stream_metrics
            .queue_high_watermark
            .fetch_max(depth, Ordering::Relaxed);
        self.inner
            .stream_metrics
            .enqueued_transitions
            .fetch_add(1, Ordering::Relaxed);
    }

    pub async fn snapshot(
        &self,
        project_id: &str,
        root_session_id: &str,
    ) -> Result<SessionControlPlaneSnapshot, String> {
        self.inner
            .data_plane
            .snapshot(project_id, root_session_id)
            .await
    }

    pub async fn runtime_metrics(&self) -> SessionControlPlaneRuntimeMetricsSnapshot {
        let mut snapshot = self.inner.data_plane.runtime_metrics().await;
        snapshot.queue_capacity = self.inner.transition_sender.max_capacity() as u64;
        snapshot.queue_depth = self
            .inner
            .stream_metrics
            .queue_depth
            .load(Ordering::Relaxed);
        snapshot.queue_high_watermark = self
            .inner
            .stream_metrics
            .queue_high_watermark
            .load(Ordering::Relaxed);
        snapshot.enqueued_transitions = self
            .inner
            .stream_metrics
            .enqueued_transitions
            .load(Ordering::Relaxed);
        snapshot.completed_transitions = self
            .inner
            .stream_metrics
            .completed_transitions
            .load(Ordering::Relaxed);
        snapshot.drained_transitions = self
            .inner
            .stream_metrics
            .drained_transitions
            .load(Ordering::Relaxed);
        snapshot.cancelled_transitions = self
            .inner
            .stream_metrics
            .cancelled_transitions
            .load(Ordering::Relaxed);
        snapshot.actor_starts = self
            .inner
            .stream_metrics
            .actor_starts
            .load(Ordering::Relaxed);
        snapshot.actor_stops = self
            .inner
            .stream_metrics
            .actor_stops
            .load(Ordering::Relaxed);
        snapshot
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.inner
            .shutdown_sender
            .send(true)
            .map_err(|_| "session control-plane transition stream already stopped".to_owned())?;
        if let Some(task) = self.inner.task.lock().await.take() {
            task.await.map_err(|error| {
                format!("session control-plane transition task failed: {error}")
            })?;
        }
        Ok(())
    }
}

impl SessionControlPlaneRuntimeRegistry {
    pub async fn runtime_for_client_dir(
        &self,
        client_dir: impl AsRef<Path>,
    ) -> Result<Arc<SessionControlPlaneRuntime>, String> {
        let client_dir = client_dir.as_ref().to_path_buf();
        let cell = if let Some(cell) = self.entries.read().await.get(&client_dir).cloned() {
            cell
        } else {
            self.entries
                .write()
                .await
                .entry(client_dir.clone())
                .or_default()
                .clone()
        };
        let runtime = cell
            .get_or_init(|| async {
                SessionControlPlaneRuntime::start_in_client_dir(&client_dir)
                    .await
                    .map(Arc::new)
            })
            .await
            .clone();
        if runtime.is_err() {
            let mut entries = self.entries.write().await;
            if entries
                .get(&client_dir)
                .is_some_and(|current| Arc::ptr_eq(current, &cell))
            {
                entries.remove(&client_dir);
            }
        }
        runtime
    }

    pub async fn shutdown_all(&self) -> Result<(), String> {
        let entries = {
            let mut entries = self.entries.write().await;
            std::mem::take(&mut *entries)
        };
        let errors = tokio_stream::iter(entries.into_values())
            .then(|cell| async move {
                match cell.get() {
                    Some(Ok(runtime)) => runtime.shutdown().await.err(),
                    Some(Err(_)) | None => None,
                }
            })
            .filter_map(|error| error)
            .collect::<Vec<_>>()
            .await;
        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "session control-plane shutdown failures: {}",
                errors.join("; ")
            ))
        }
    }

    pub async fn resident_workspace_count(&self) -> usize {
        self.entries.read().await.len()
    }
}

async fn run_session_control_plane_transition_stream(
    data_plane: SessionControlPlaneTransactionOwner,
    mut transitions: ReceiverStream<SessionControlPlaneTransition>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
    stream_metrics: Arc<SessionControlPlaneStreamMetrics>,
) {
    stream_metrics.actor_starts.fetch_add(1, Ordering::Relaxed);
    loop {
        tokio::select! {
            biased;
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    transitions.close();
                    break;
                }
            }
            transition = transitions.next() => {
                let Some(transition) = transition else {
                    break;
                };
                record_transition_started(&stream_metrics);
                let reply_delivered =
                    execute_session_control_plane_transition(&data_plane, transition).await;
                record_transition_completed(&stream_metrics, reply_delivered);
            }
        }
    }
    while let Some(transition) = transitions.next().await {
        record_transition_started(&stream_metrics);
        let reply_delivered =
            execute_session_control_plane_transition(&data_plane, transition).await;
        stream_metrics
            .drained_transitions
            .fetch_add(1, Ordering::Relaxed);
        record_transition_completed(&stream_metrics, reply_delivered);
    }
    stream_metrics.actor_stops.fetch_add(1, Ordering::Relaxed);
}

fn record_transition_started(metrics: &SessionControlPlaneStreamMetrics) {
    metrics.queue_depth.fetch_sub(1, Ordering::Relaxed);
}

fn record_transition_completed(metrics: &SessionControlPlaneStreamMetrics, reply_delivered: bool) {
    metrics
        .completed_transitions
        .fetch_add(1, Ordering::Relaxed);
    if !reply_delivered {
        metrics
            .cancelled_transitions
            .fetch_add(1, Ordering::Relaxed);
    }
}

async fn execute_session_control_plane_transition(
    data_plane: &SessionControlPlaneTransactionOwner,
    transition: SessionControlPlaneTransition,
) -> bool {
    match transition {
        SessionControlPlaneTransition::RegisterAgent {
            registration,
            reply,
        } => reply
            .send(data_plane.register_agent(&registration).await)
            .is_ok(),
        SessionControlPlaneTransition::AdmitDelegation { proposal, reply } => reply
            .send(data_plane.admit_delegation(&proposal).await)
            .is_ok(),
    }
}
