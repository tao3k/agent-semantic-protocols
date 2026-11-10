// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Bounded exact-generation lifecycle for Runtime-owned Search attachments.

use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use tokio::sync::broadcast;
use tokio::sync::watch;
use tokio_stream::Stream;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

pub const RUNTIME_SEARCH_ATTACHMENT_EVENT_CAPACITY: usize = 256;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeSearchDerivedAttachmentKind {
    Graph,
    Tantivy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeSearchDerivedAttachmentState {
    Queued,
    Building,
    Ready,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSearchDerivedAttachmentEvent {
    pub schema_id: &'static str,
    pub schema_version: &'static str,
    pub sequence: u64,
    pub project_id: String,
    pub workspace_id: String,
    pub generation_token: u64,
    pub content_generation_digest: String,
    pub attachment: RuntimeSearchDerivedAttachmentKind,
    pub state: RuntimeSearchDerivedAttachmentState,
    pub build_micros: Option<u64>,
    pub finalize_micros: Option<u64>,
    pub reason_kind: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSearchDerivedAttachmentIdentity {
    pub project_id: String,
    pub workspace_id: String,
    pub generation_token: u64,
    pub content_generation_digest: String,
    pub attachment: RuntimeSearchDerivedAttachmentKind,
}

pub type RuntimeSearchDerivedAttachmentKey = (String, String, RuntimeSearchDerivedAttachmentKind);
pub type RuntimeSearchDerivedAttachmentSnapshot =
    BTreeMap<RuntimeSearchDerivedAttachmentKey, RuntimeSearchDerivedAttachmentEvent>;

#[derive(Clone)]
pub struct RuntimeSearchDerivedAttachmentHub {
    snapshot: watch::Sender<Arc<RuntimeSearchDerivedAttachmentSnapshot>>,
    events: broadcast::Sender<RuntimeSearchDerivedAttachmentEvent>,
    next_sequence: Arc<AtomicU64>,
}

impl RuntimeSearchDerivedAttachmentHub {
    #[must_use]
    pub fn new() -> Self {
        let (snapshot, _) = watch::channel(Arc::new(RuntimeSearchDerivedAttachmentSnapshot::new()));
        let (events, _) = broadcast::channel(RUNTIME_SEARCH_ATTACHMENT_EVENT_CAPACITY);
        Self {
            snapshot,
            events,
            next_sequence: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn publish(
        &self,
        identity: &RuntimeSearchDerivedAttachmentIdentity,
        state: RuntimeSearchDerivedAttachmentState,
        timing: Option<(u64, u64)>,
        reason_kind: Option<&'static str>,
    ) -> bool {
        let sequence = self.next_sequence.fetch_add(1, Ordering::AcqRel) + 1;
        let event = RuntimeSearchDerivedAttachmentEvent {
            schema_id: "agent.semantic-protocols.runtime-search-derived-attachment-event",
            schema_version: "1",
            sequence,
            project_id: identity.project_id.clone(),
            workspace_id: identity.workspace_id.clone(),
            generation_token: identity.generation_token,
            content_generation_digest: identity.content_generation_digest.clone(),
            attachment: identity.attachment,
            state,
            build_micros: timing.map(|value| value.0),
            finalize_micros: timing.map(|value| value.1),
            reason_kind,
        };
        let key = (
            identity.project_id.clone(),
            identity.workspace_id.clone(),
            identity.attachment,
        );
        let mut accepted = false;
        self.snapshot.send_modify(|snapshot| {
            let mut next = snapshot.as_ref().clone();
            if let Some(current) = next.get(&key) {
                if current.generation_token > event.generation_token
                    || (current.generation_token == event.generation_token
                        && (current.content_generation_digest != event.content_generation_digest
                            || !transition_admitted(current.state, event.state)))
                {
                    return;
                }
            } else if event.state != RuntimeSearchDerivedAttachmentState::Queued {
                return;
            }
            next.insert(key, event.clone());
            *snapshot = Arc::new(next);
            accepted = true;
        });
        if accepted {
            let _ = self.events.send(event);
        }
        accepted
    }

    pub fn subscribe(
        &self,
    ) -> Pin<Box<dyn Stream<Item = Result<RuntimeSearchDerivedAttachmentEvent, String>> + Send>>
    {
        Box::pin(BroadcastStream::new(self.events.subscribe()).map(|event| {
            event.map_err(|error| format!("Runtime search attachment event stream lagged: {error}"))
        }))
    }

    #[must_use]
    pub fn snapshot(&self) -> Arc<RuntimeSearchDerivedAttachmentSnapshot> {
        self.snapshot.borrow().clone()
    }
}

impl Default for RuntimeSearchDerivedAttachmentHub {
    fn default() -> Self {
        Self::new()
    }
}

fn transition_admitted(
    current: RuntimeSearchDerivedAttachmentState,
    next: RuntimeSearchDerivedAttachmentState,
) -> bool {
    use RuntimeSearchDerivedAttachmentState::Building;
    use RuntimeSearchDerivedAttachmentState::Failed;
    use RuntimeSearchDerivedAttachmentState::Queued;
    use RuntimeSearchDerivedAttachmentState::Ready;
    matches!(
        (current, next),
        (Queued, Queued | Building | Failed)
            | (Building, Building | Ready | Failed)
            | (Ready, Ready)
            | (Failed, Failed)
    )
}
