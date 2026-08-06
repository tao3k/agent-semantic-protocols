//! Owns the Runtime Server's in-memory Graph Turbo resident status signal.

use std::sync::{Arc, RwLock};

use tokio::sync::watch;

use crate::runtime_server_control::{GraphTurboResidentState, GraphTurboResidentStatus};

#[derive(Clone)]
pub struct GraphTurboResidentStatusHandle {
    value: Arc<RwLock<GraphTurboResidentStatus>>,
    generation: watch::Sender<u64>,
}

impl GraphTurboResidentStatusHandle {
    pub fn new(status: GraphTurboResidentStatus) -> Self {
        let (generation, _) = watch::channel(0);
        Self {
            value: Arc::new(RwLock::new(status)),
            generation,
        }
    }

    pub fn snapshot(&self) -> GraphTurboResidentStatus {
        self.value
            .read()
            .map(|status| status.clone())
            .unwrap_or_else(|_| GraphTurboResidentStatus {
                state: GraphTurboResidentState::Failed,
                process_id: None,
                runtime_artifact: None,
                execution_command_digest: None,
                reason: Some("Graph Turbo resident status lock poisoned".to_owned()),
            })
    }

    pub fn update(&self, status: GraphTurboResidentStatus) {
        if let Ok(mut current) = self.value.write() {
            *current = status;
            self.advance_generation();
        }
    }

    pub fn mutate(&self, update: impl FnOnce(&mut GraphTurboResidentStatus)) {
        if let Ok(mut current) = self.value.write() {
            update(&mut current);
            self.advance_generation();
        }
    }

    pub(crate) fn shared(&self) -> Arc<RwLock<GraphTurboResidentStatus>> {
        Arc::clone(&self.value)
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.generation.subscribe()
    }

    fn advance_generation(&self) {
        self.generation.send_modify(|generation| {
            *generation = generation.saturating_add(1);
        });
    }
}
