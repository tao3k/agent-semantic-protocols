// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Bounded LRU registry for content-bound Runtime client sessions.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use crate::runtime_language_client::{AspClientRuntimeHandoff, SessionKey};

impl SessionKey {
    pub(crate) fn from_publication(
        publication: &AspClientRuntimeHandoff,
        project_id: String,
        workspace_id: String,
    ) -> Self {
        Self {
            project_id,
            workspace_id,
            publication_nonce: publication.publication_nonce.clone(),
            binary_content_digest: publication.artifact_digest.to_string(),
        }
    }

    #[cfg(test)]
    pub(crate) fn fixture(identity: u64) -> Self {
        Self {
            project_id: format!("repo-{identity}"),
            workspace_id: format!("workspace-{identity}"),
            publication_nonce: format!("publication-{identity}"),
            binary_content_digest: format!("blake3-256:{:064x}", identity + 2),
        }
    }

    #[cfg(test)]
    pub(crate) fn fixture_successor(identity: u64) -> Self {
        let mut key = Self::fixture(identity);
        key.publication_nonce = format!("publication-successor-{identity}");
        key
    }
}

pub(crate) struct SessionRegistry<T> {
    capacity: usize,
    entries: HashMap<SessionKey, Arc<tokio::sync::OnceCell<Arc<T>>>>,
    lru: VecDeque<SessionKey>,
}

impl<T> SessionRegistry<T> {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: HashMap::new(),
            lru: VecDeque::new(),
        }
    }

    fn touch(&mut self, key: &SessionKey) {
        if let Some(index) = self.lru.iter().position(|candidate| candidate == key) {
            self.lru.remove(index);
        }
        self.lru.push_back(key.clone());
    }

    fn evictable(cell: &tokio::sync::OnceCell<Arc<T>>) -> bool {
        cell.get()
            .is_some_and(|value| Arc::strong_count(value) == 1)
    }

    fn evict_one_idle(&mut self) -> bool {
        let Some(index) = self.lru.iter().position(|key| {
            self.entries
                .get(key)
                .is_some_and(|cell| Self::evictable(cell))
        }) else {
            return false;
        };
        let key = self.lru.remove(index).expect("LRU index was present");
        self.entries.remove(&key);
        true
    }

    pub(crate) fn reserve(
        &mut self,
        key: SessionKey,
    ) -> Result<Arc<tokio::sync::OnceCell<Arc<T>>>, String> {
        if let Some(cell) = self.entries.get(&key).cloned() {
            self.touch(&key);
            return Ok(cell);
        }
        while self.entries.len() >= self.capacity {
            if !self.evict_one_idle() {
                return Err(format!(
                    "reasonKind=runtime-client-session-capacity-exhausted capacity={} activeOrConnecting={}",
                    self.capacity,
                    self.entries.len()
                ));
            }
        }
        let cell = Arc::new(tokio::sync::OnceCell::new());
        self.entries.insert(key.clone(), Arc::clone(&cell));
        self.touch(&key);
        Ok(cell)
    }

    pub(crate) fn remove_if_same(
        &mut self,
        key: &SessionKey,
        expected: &Arc<tokio::sync::OnceCell<Arc<T>>>,
    ) {
        if self
            .entries
            .get(key)
            .is_some_and(|current| Arc::ptr_eq(current, expected))
        {
            self.entries.remove(key);
            if let Some(index) = self.lru.iter().position(|candidate| candidate == key) {
                self.lru.remove(index);
            }
        }
    }

    pub(crate) fn remove_key(&mut self, key: &SessionKey) -> bool {
        let removed = self.entries.remove(key).is_some();
        if let Some(index) = self.lru.iter().position(|candidate| candidate == key) {
            self.lru.remove(index);
        }
        removed
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn drain_idle(&mut self) -> usize {
        let before = self.entries.len();
        while self.evict_one_idle() {}
        before - self.entries.len()
    }
}
