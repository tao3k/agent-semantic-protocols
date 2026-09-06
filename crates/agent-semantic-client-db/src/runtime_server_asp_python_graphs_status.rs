// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Owns the Runtime Server's in-memory asp-python-graphs status signal.

use std::sync::{Arc, RwLock};

use tokio::sync::watch;

use crate::runtime_server_control::{AspPythonGraphsState, AspPythonGraphsStatus};

#[derive(Clone)]
pub struct AspPythonGraphsStatusHandle {
    value: Arc<RwLock<AspPythonGraphsStatus>>,
    generation: watch::Sender<u64>,
}

impl AspPythonGraphsStatusHandle {
    pub fn new(status: AspPythonGraphsStatus) -> Self {
        let (generation, _) = watch::channel(0);
        Self {
            value: Arc::new(RwLock::new(status)),
            generation,
        }
    }

    pub fn snapshot(&self) -> AspPythonGraphsStatus {
        self.value
            .read()
            .map(|status| status.clone())
            .unwrap_or_else(|_| AspPythonGraphsStatus {
                state: AspPythonGraphsState::Failed,
                process_id: None,
                runtime_artifact: None,
                execution_command_digest: None,
                reason: Some("asp-python-graphs status lock poisoned".to_owned()),
            })
    }

    pub fn update(&self, status: AspPythonGraphsStatus) {
        if let Ok(mut current) = self.value.write() {
            *current = status;
            self.advance_generation();
        }
    }

    pub fn mutate(&self, update: impl FnOnce(&mut AspPythonGraphsStatus)) {
        if let Ok(mut current) = self.value.write() {
            update(&mut current);
            self.advance_generation();
        }
    }

    pub(crate) fn shared(&self) -> Arc<RwLock<AspPythonGraphsStatus>> {
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
