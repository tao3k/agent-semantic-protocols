// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Content-addressed provider parser products, independent of generation proof.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use agent_semantic_provider_transport::projection_batch::ProviderProjectedOwner;
use serde::{Deserialize, Serialize};

const PARSER_ARTIFACT_SCHEMA_ID: &str = "agent.semantic-protocols.provider-parser-owner-artifact";
const PARSER_ARTIFACT_SCHEMA_VERSION: &str = "1";
const MAX_PARSER_ARTIFACT_BYTES: usize = 32 * 1024 * 1024;
const MAX_RESIDENT_ARTIFACT_BYTES: usize = 64 * 1024 * 1024;
const MAX_RESIDENT_ARTIFACT_ENTRIES: usize = 4_096;
static NEXT_PENDING_ID: AtomicU64 = AtomicU64::new(0);

fn resident_charge_bytes(encoded_bytes: usize) -> usize {
    // The decoded owner retains Strings, Vecs, and allocation metadata in
    // addition to the encoded payload. A 2x charge with a page-sized floor is
    // deliberately conservative; the cache budget must bound retained heap,
    // not merely count bytes that used to live on disk.
    encoded_bytes.saturating_mul(2).max(4_096)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ParserArtifactIdentity {
    pub provider_id: String,
    pub parser_identity_digest: String,
    pub query_pack_digest: String,
    pub auxiliary_input_digest: String,
    pub owner_path: String,
    pub owner_content_digest: String,
}

impl ParserArtifactIdentity {
    pub(super) fn key_digest(&self) -> Result<String, String> {
        let bytes = serde_json::to_vec(self)
            .map_err(|error| format!("encode parser artifact identity: {error}"))?;
        Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ParserArtifactBody {
    identity: ParserArtifactIdentity,
    owner: ProviderProjectedOwner,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ParserArtifactEnvelope {
    schema_id: String,
    schema_version: String,
    key_digest: String,
    artifact_digest: String,
    body: ParserArtifactBody,
}

#[derive(Clone)]
pub struct ParserArtifactResidentCache {
    inner: Arc<parking_lot::Mutex<ParserArtifactResidentCacheState>>,
    byte_budget: usize,
    entry_budget: usize,
}

#[derive(Default)]
struct ParserArtifactResidentCacheState {
    revision: u64,
    resident_bytes: usize,
    entries: HashMap<String, ParserArtifactResidentEntry>,
    recency: BTreeMap<u64, String>,
}

struct ParserArtifactResidentEntry {
    identity: ParserArtifactIdentity,
    owner: ProviderProjectedOwner,
    charge_bytes: usize,
    last_used: u64,
}

pub(super) enum ParserArtifactReuse {
    Miss,
    Resident(ProviderProjectedOwner),
    Persistent(ProviderProjectedOwner),
}

impl ParserArtifactResidentCache {
    #[must_use]
    pub fn for_process_memory_budget(process_memory_budget_bytes: usize) -> Self {
        let process_memory_budget_bytes = process_memory_budget_bytes.max(1);
        let byte_budget = (process_memory_budget_bytes / 8)
            .clamp(1, MAX_RESIDENT_ARTIFACT_BYTES)
            .min(process_memory_budget_bytes);
        let entry_budget = (byte_budget / 4_096).clamp(1, MAX_RESIDENT_ARTIFACT_ENTRIES);
        Self::new(byte_budget, entry_budget)
    }

    #[must_use]
    pub fn new(byte_budget: usize, entry_budget: usize) -> Self {
        Self {
            inner: Arc::new(parking_lot::Mutex::new(
                ParserArtifactResidentCacheState::default(),
            )),
            byte_budget: byte_budget.max(1),
            entry_budget: entry_budget.max(1),
        }
    }

    fn get(
        &self,
        key_digest: &str,
        identity: &ParserArtifactIdentity,
    ) -> Result<Option<ProviderProjectedOwner>, String> {
        let mut state = self.inner.lock();
        let Some(mut entry) = state.entries.remove(key_digest) else {
            return Ok(None);
        };
        state.recency.remove(&entry.last_used);
        if entry.identity != *identity
            || entry.owner.owner_path != identity.owner_path
            || !same_digest(
                &entry.owner.source_leaf_digest,
                &identity.owner_content_digest,
            )
        {
            state.resident_bytes = state.resident_bytes.saturating_sub(entry.charge_bytes);
            return Err("resident parser artifact identity or content digest drift".to_owned());
        }
        let revision = state.next_revision();
        entry.last_used = revision;
        let owner = entry.owner.clone();
        state.recency.insert(revision, key_digest.to_owned());
        state.entries.insert(key_digest.to_owned(), entry);
        Ok(Some(owner))
    }

    fn insert(
        &self,
        key_digest: String,
        identity: ParserArtifactIdentity,
        owner: ProviderProjectedOwner,
        charge_bytes: usize,
    ) {
        if charge_bytes > self.byte_budget {
            return;
        }
        let mut state = self.inner.lock();
        state.remove(&key_digest);
        let revision = state.next_revision();
        state.resident_bytes = state.resident_bytes.saturating_add(charge_bytes);
        state.recency.insert(revision, key_digest.clone());
        state.entries.insert(
            key_digest,
            ParserArtifactResidentEntry {
                identity,
                owner,
                charge_bytes,
                last_used: revision,
            },
        );
        while state.resident_bytes > self.byte_budget || state.entries.len() > self.entry_budget {
            let Some((&oldest, key)) = state.recency.first_key_value() else {
                break;
            };
            let key = key.clone();
            state.recency.remove(&oldest);
            if let Some(entry) = state.entries.remove(&key) {
                state.resident_bytes = state.resident_bytes.saturating_sub(entry.charge_bytes);
            }
        }
    }

    #[cfg(test)]
    fn resident_usage(&self) -> (usize, usize) {
        let state = self.inner.lock();
        (state.resident_bytes, state.entries.len())
    }
}

impl ParserArtifactResidentCacheState {
    fn next_revision(&mut self) -> u64 {
        if self.revision == u64::MAX {
            self.entries.clear();
            self.recency.clear();
            self.resident_bytes = 0;
            self.revision = 0;
        }
        self.revision += 1;
        self.revision
    }

    fn remove(&mut self, key_digest: &str) {
        if let Some(entry) = self.entries.remove(key_digest) {
            self.recency.remove(&entry.last_used);
            self.resident_bytes = self.resident_bytes.saturating_sub(entry.charge_bytes);
        }
    }
}

#[derive(Clone)]
pub(super) struct ParserArtifactStore {
    root: PathBuf,
    resident_cache: Option<ParserArtifactResidentCache>,
}

impl ParserArtifactStore {
    #[cfg(test)]
    pub(super) fn for_client_db(db_path: &Path) -> Result<Self, String> {
        let parent = db_path.parent().ok_or_else(|| {
            format!(
                "Client DB path has no parser artifact parent: {}",
                db_path.display()
            )
        })?;
        Ok(Self::for_artifact_root(parent))
    }

    pub(super) fn for_artifact_root(root: &Path) -> Self {
        Self {
            root: root.join("provider-parser-artifacts-v1"),
            resident_cache: None,
        }
    }

    pub(super) fn for_artifact_root_with_resident_cache(
        root: &Path,
        resident_cache: ParserArtifactResidentCache,
    ) -> Self {
        Self {
            root: root.join("provider-parser-artifacts-v1"),
            resident_cache: Some(resident_cache),
        }
    }

    pub(super) async fn read(
        &self,
        identity: &ParserArtifactIdentity,
    ) -> Result<ParserArtifactReuse, String> {
        let key_digest = identity.key_digest()?;
        if let Some(cache) = &self.resident_cache
            && let Some(owner) = cache.get(&key_digest, identity)?
        {
            return Ok(ParserArtifactReuse::Resident(owner));
        }
        let path = self.path_for_digest(&key_digest)?;
        let bytes = match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ParserArtifactReuse::Miss);
            }
            Err(error) => {
                return Err(format!(
                    "read parser artifact `{}`: {error}",
                    path.display()
                ));
            }
        };
        if bytes.len() > MAX_PARSER_ARTIFACT_BYTES {
            return Err(format!(
                "parser artifact exceeds byte budget: path={} bytes={} limit={MAX_PARSER_ARTIFACT_BYTES}",
                path.display(),
                bytes.len(),
            ));
        }
        let envelope: ParserArtifactEnvelope = serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode parser artifact `{}`: {error}", path.display()))?;
        envelope.validate(identity, &key_digest)?;
        let owner = envelope.body.owner;
        if let Some(cache) = &self.resident_cache {
            cache.insert(
                key_digest,
                identity.clone(),
                owner.clone(),
                resident_charge_bytes(bytes.len()),
            );
        }
        Ok(ParserArtifactReuse::Persistent(owner))
    }

    pub(super) async fn publish(
        &self,
        identity: ParserArtifactIdentity,
        owner: ProviderProjectedOwner,
    ) -> Result<(), String> {
        let key_digest = identity.key_digest()?;
        let body = ParserArtifactBody { identity, owner };
        let artifact_digest = body_digest(&body)?;
        let envelope = ParserArtifactEnvelope {
            schema_id: PARSER_ARTIFACT_SCHEMA_ID.to_owned(),
            schema_version: PARSER_ARTIFACT_SCHEMA_VERSION.to_owned(),
            key_digest: key_digest.clone(),
            artifact_digest,
            body,
        };
        envelope.validate(&envelope.body.identity, &key_digest)?;
        let bytes = serde_json::to_vec(&envelope)
            .map_err(|error| format!("encode parser artifact: {error}"))?;
        if bytes.len() > MAX_PARSER_ARTIFACT_BYTES {
            return Err(format!(
                "parser artifact exceeds byte budget: bytes={} limit={MAX_PARSER_ARTIFACT_BYTES}",
                bytes.len()
            ));
        }
        tokio::fs::create_dir_all(&self.root)
            .await
            .map_err(|error| {
                format!(
                    "create parser artifact directory `{}`: {error}",
                    self.root.display()
                )
            })?;
        let final_path = self.path_for_digest(&key_digest)?;
        if let Ok(existing) = tokio::fs::read(&final_path).await
            && existing == bytes
        {
            if let Some(cache) = &self.resident_cache {
                cache.insert(
                    key_digest,
                    envelope.body.identity.clone(),
                    envelope.body.owner.clone(),
                    resident_charge_bytes(bytes.len()),
                );
            }
            return Ok(());
        }
        let pending_id = NEXT_PENDING_ID.fetch_add(1, Ordering::Relaxed);
        let pending_path = self.root.join(format!(
            ".{}.{}.{}.pending",
            std::process::id(),
            pending_id,
            final_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("parser-artifact")
        ));
        tokio::fs::write(&pending_path, &bytes)
            .await
            .map_err(|error| {
                format!(
                    "write parser artifact `{}`: {error}",
                    pending_path.display()
                )
            })?;
        tokio::fs::rename(&pending_path, &final_path)
            .await
            .map_err(|error| {
                format!(
                    "publish parser artifact `{}`: {error}",
                    final_path.display()
                )
            })?;
        if let Some(cache) = &self.resident_cache {
            cache.insert(
                key_digest,
                envelope.body.identity,
                envelope.body.owner,
                resident_charge_bytes(bytes.len()),
            );
        }
        Ok(())
    }

    fn path_for_digest(&self, digest: &str) -> Result<PathBuf, String> {
        let value = digest
            .strip_prefix("blake3-256:")
            .ok_or_else(|| "parser artifact key is not canonical".to_owned())?;
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err("parser artifact key is not canonical".to_owned());
        }
        Ok(self.root.join(format!("{value}.json")))
    }
}

impl ParserArtifactEnvelope {
    fn validate(
        &self,
        expected_identity: &ParserArtifactIdentity,
        expected_key_digest: &str,
    ) -> Result<(), String> {
        if self.schema_id != PARSER_ARTIFACT_SCHEMA_ID
            || self.schema_version != PARSER_ARTIFACT_SCHEMA_VERSION
            || &self.body.identity != expected_identity
            || self.key_digest != expected_key_digest
            || self.body.identity.key_digest()? != self.key_digest
            || body_digest(&self.body)? != self.artifact_digest
            || self.body.owner.owner_path != self.body.identity.owner_path
            || !same_digest(
                &self.body.owner.source_leaf_digest,
                &self.body.identity.owner_content_digest,
            )
        {
            return Err("parser artifact identity or content digest drift".to_owned());
        }
        Ok(())
    }
}

fn body_digest(body: &ParserArtifactBody) -> Result<String, String> {
    let bytes = serde_json::to_vec(body)
        .map_err(|error| format!("encode parser artifact body: {error}"))?;
    Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
}

fn same_digest(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix("blake3-256:")
            .is_some_and(|digest| digest == right)
        || right
            .strip_prefix("blake3-256:")
            .is_some_and(|digest| digest == left)
}

#[cfg(test)]
#[path = "../../tests/unit/parser_artifact_store.rs"]
mod tests;
