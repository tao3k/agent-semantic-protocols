// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Single-flight Search and Query materialization state transitions.

use std::sync::Arc;

use super::runtime_query_generation_model::{
    RuntimeQueryGeneration, RuntimeQueryMaterializationState, RuntimeQueryTerminalState,
    RuntimeSearchMaterializationState, RuntimeSearchTerminalState,
};

impl RuntimeQueryGeneration {
    pub(crate) fn spawn_materialization<F>(
        &self,
        name: &'static str,
        future: F,
    ) -> Result<(), String>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let permit = self.task_scope.permit(name)?;
        let mut tasks = self
            .materialization_tasks
            .lock()
            .map_err(|_| "Query materialization task registry is poisoned".to_owned())?;
        while tasks.try_join_next().is_some() {}
        tasks.spawn(async move {
            future.await;
            permit.complete();
        });
        Ok(())
    }

    pub(crate) fn search_materialization(
        &self,
        key: &str,
    ) -> Result<Option<RuntimeSearchMaterializationState>, String> {
        Ok(self
            .search_materializations
            .lock()
            .map_err(|_| "Runtime Search materialization cache poisoned".to_owned())?
            .get(key)
            .cloned())
    }

    /// Atomically claim one generation-bound Search materialization.
    pub(crate) fn begin_search_materialization(&self, key: String) -> Result<bool, String> {
        let mut materializations = self
            .search_materializations
            .lock()
            .map_err(|_| "Runtime Search materialization cache poisoned".to_owned())?;
        if materializations.contains_key(&key) {
            return Ok(false);
        }
        materializations.insert(
            key,
            RuntimeSearchMaterializationState::Building(Arc::new(
                tokio::sync::watch::channel(None).0,
            )),
        );
        Ok(true)
    }

    pub(crate) fn publish_search_materialization(
        &self,
        key: String,
        result: Result<serde_json::Value, agent_semantic_client_server::AspClientDispatchError>,
    ) -> Result<(), String> {
        let mut materializations = self
            .search_materializations
            .lock()
            .map_err(|_| "Runtime Search materialization cache poisoned".to_owned())?;
        let Some(RuntimeSearchMaterializationState::Building(completion)) =
            materializations.get(&key)
        else {
            return Err("Runtime Search materialization lost its Building claim".to_owned());
        };
        let completion = Arc::clone(completion);
        let terminal = match result {
            Ok(value) => RuntimeSearchTerminalState::Ready(Arc::new(value)),
            Err(error) => RuntimeSearchTerminalState::Failed(Arc::new(error)),
        };
        materializations.remove(&key);
        let terminal_capacity = self.resource_supervisor.effective_cpu().max(2);
        while materializations
            .values()
            .filter(|state| !matches!(state, RuntimeSearchMaterializationState::Building(_)))
            .count()
            >= terminal_capacity
        {
            let evicted = materializations
                .iter()
                .filter(|(_, state)| {
                    !matches!(state, RuntimeSearchMaterializationState::Building(_))
                })
                .map(|(key, _)| key)
                .min()
                .cloned();
            let Some(evicted) = evicted else { break };
            materializations.remove(&evicted);
        }
        materializations.insert(key, terminal.clone().materialization_state());
        completion.send_replace(Some(terminal));
        Ok(())
    }

    pub(crate) async fn await_search_materialization(
        &self,
        key: &str,
    ) -> Result<RuntimeSearchTerminalState, String> {
        match self.search_materialization(key)? {
            Some(RuntimeSearchMaterializationState::Building(completion)) => {
                let mut receiver = completion.subscribe();
                if receiver.borrow_and_update().is_none() {
                    receiver
                        .changed()
                        .await
                        .map_err(|_| "Search completion channel closed".to_owned())?;
                }
                receiver
                    .borrow()
                    .clone()
                    .ok_or_else(|| "Search completion did not publish a terminal".to_owned())
            }
            Some(RuntimeSearchMaterializationState::Ready(value)) => {
                Ok(RuntimeSearchTerminalState::Ready(value))
            }
            Some(RuntimeSearchMaterializationState::Failed(error)) => {
                Ok(RuntimeSearchTerminalState::Failed(error))
            }
            None => Err("Search materialization has no claim".to_owned()),
        }
    }

    pub(crate) fn query_materialization(
        &self,
        key: &str,
    ) -> Result<Option<RuntimeQueryMaterializationState>, String> {
        Ok(self
            .query_materializations
            .lock()
            .map_err(|_| "Runtime Query materialization cache poisoned".to_owned())?
            .get(key)
            .cloned())
    }

    /// Atomically claim one generation-bound Query materialization.
    pub(crate) fn begin_query_materialization(&self, key: String) -> Result<bool, String> {
        let mut materializations = self
            .query_materializations
            .lock()
            .map_err(|_| "Runtime Query materialization cache poisoned".to_owned())?;
        if materializations.contains_key(&key) {
            return Ok(false);
        }
        materializations.insert(
            key,
            RuntimeQueryMaterializationState::Building(Arc::new(
                tokio::sync::watch::channel(None).0,
            )),
        );
        Ok(true)
    }

    pub(crate) fn publish_query_materialization(
        &self,
        key: String,
        result: Result<serde_json::Value, agent_semantic_client_server::AspClientDispatchError>,
    ) -> Result<(), String> {
        let terminal_slot = query_materialization_slot(&key)?;
        let mut materializations = self
            .query_materializations
            .lock()
            .map_err(|_| "Runtime Query materialization cache poisoned".to_owned())?;
        let Some(RuntimeQueryMaterializationState::Building(completion)) =
            materializations.get(&key)
        else {
            return Err("Runtime Query materialization lost its Building claim".to_owned());
        };
        let completion = Arc::clone(completion);
        let terminal = match result {
            Ok(value) => RuntimeQueryTerminalState::Ready(Arc::new(value)),
            Err(error) => RuntimeQueryTerminalState::Failed(Arc::new(error)),
        };
        materializations.remove(&key);
        materializations.retain(|existing_key, state| {
            matches!(state, RuntimeQueryMaterializationState::Building(_))
                || query_materialization_slot(existing_key).ok() != Some(terminal_slot)
        });
        materializations.insert(key, terminal.clone().materialization_state());
        completion.send_replace(Some(terminal));
        Ok(())
    }

    pub(crate) async fn await_query_materialization(
        &self,
        key: &str,
    ) -> Result<RuntimeQueryTerminalState, String> {
        match self.query_materialization(key)? {
            Some(RuntimeQueryMaterializationState::Building(completion)) => {
                let mut receiver = completion.subscribe();
                if receiver.borrow_and_update().is_none() {
                    receiver
                        .changed()
                        .await
                        .map_err(|_| "Query completion channel closed".to_owned())?;
                }
                receiver
                    .borrow()
                    .clone()
                    .ok_or_else(|| "Query completion did not publish a terminal".to_owned())
            }
            Some(RuntimeQueryMaterializationState::Ready(value)) => {
                Ok(RuntimeQueryTerminalState::Ready(value))
            }
            Some(RuntimeQueryMaterializationState::Failed(error)) => {
                Ok(RuntimeQueryTerminalState::Failed(error))
            }
            None => Err("Query materialization has no claim".to_owned()),
        }
    }
}

fn query_materialization_slot(key: &str) -> Result<&str, String> {
    let slot = key
        .split_once('\0')
        .map(|(slot, _)| slot)
        .ok_or_else(|| "Runtime Query materialization key has no projection slot".to_owned())?;
    match slot {
        "source" | "callable-skeleton" => Ok(slot),
        _ => Err("Runtime Query materialization key has no V1 projection slot".to_owned()),
    }
}
