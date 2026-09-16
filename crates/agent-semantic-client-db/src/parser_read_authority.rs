// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    future::Future,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::{OnceCell, RwLock};

pub const PARSER_READ_AUTHORITY_SCHEMA_ID: &str = "agent.semantic-protocols.parser-read-authority";
pub const PARSER_READ_AUTHORITY_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeEndpointState {
    Healthy,
    Stopped,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParserReadRoute {
    ResidentMemory,
    DurableMmap,
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParserReadState {
    Ready,
    GenerationRequired,
    GenerationStale,
    GenerationCorrupt,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParserReadCounters {
    pub mapped_bytes: u64,
    pub database_opens: u64,
    pub provider_spawns: u64,
    pub source_file_reads: u64,
    pub workspace_canonicalizations: u64,
    pub activation_refreshes: u64,
    pub writes: u64,
}

impl ParserReadCounters {
    pub fn validate_read_only(self) -> Result<Self, String> {
        if self.database_opens != 0
            || self.provider_spawns != 0
            || self.source_file_reads != 0
            || self.workspace_canonicalizations != 0
            || self.activation_refreshes != 0
            || self.writes != 0
        {
            return Err(
                "parser read authority performed a forbidden mutation or cold source operation"
                    .to_owned(),
            );
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParserReadContext {
    pub project_id: String,
    pub workspace_id: String,
    pub canonical_workspace_root: String,
    pub language_id: String,
    pub provider_id: String,
    pub provider_manifest_digest: String,
}

impl ParserReadContext {
    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("projectId", self.project_id.as_str()),
            ("workspaceId", self.workspace_id.as_str()),
            ("languageId", self.language_id.as_str()),
            ("providerId", self.provider_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{field} must be non-empty text"));
            }
        }
        if !Path::new(&self.canonical_workspace_root).is_absolute() {
            return Err("canonicalWorkspaceRoot must be absolute".to_owned());
        }
        validate_digest("providerManifestDigest", &self.provider_manifest_digest)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommittedParserGeneration {
    pub context: ParserReadContext,
    pub generation_epoch: u64,
    pub generation_digest: String,
    pub source_root_digest: String,
}

impl CommittedParserGeneration {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        if self.generation_epoch == 0 {
            return Err("generationEpoch must be positive".to_owned());
        }
        for (field, value) in [
            ("generationDigest", self.generation_digest.as_str()),
            ("sourceRootDigest", self.source_root_digest.as_str()),
        ] {
            validate_digest(field, value)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParserReadAuthority {
    pub schema_id: String,
    pub schema_version: String,
    pub project_id: String,
    pub workspace_id: String,
    pub canonical_workspace_root: String,
    pub language_id: String,
    pub provider_id: String,
    pub provider_manifest_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_epoch: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_root_digest: Option<String>,
    pub route: ParserReadRoute,
    pub state: ParserReadState,
    pub runtime_endpoint_state: RuntimeEndpointState,
    pub elapsed_micros: u64,
    pub counters: ParserReadCounters,
}

impl ParserReadAuthority {
    pub fn ready(
        generation: CommittedParserGeneration,
        runtime_endpoint_state: RuntimeEndpointState,
        memory_loaded: bool,
        elapsed_micros: u64,
        counters: ParserReadCounters,
    ) -> Result<Self, String> {
        generation.validate()?;
        let counters = counters.validate_read_only()?;
        Ok(Self {
            schema_id: PARSER_READ_AUTHORITY_SCHEMA_ID.to_owned(),
            schema_version: PARSER_READ_AUTHORITY_SCHEMA_VERSION.to_owned(),
            project_id: generation.context.project_id,
            workspace_id: generation.context.workspace_id,
            canonical_workspace_root: generation.context.canonical_workspace_root,
            language_id: generation.context.language_id,
            provider_id: generation.context.provider_id,
            provider_manifest_digest: generation.context.provider_manifest_digest,
            generation_epoch: Some(generation.generation_epoch),
            generation_digest: Some(generation.generation_digest),
            source_root_digest: Some(generation.source_root_digest),
            route: if memory_loaded {
                ParserReadRoute::ResidentMemory
            } else {
                ParserReadRoute::DurableMmap
            },
            state: ParserReadState::Ready,
            runtime_endpoint_state,
            elapsed_micros,
            counters,
        })
    }

    pub fn unavailable(
        context: ParserReadContext,
        generation: Option<&CommittedParserGeneration>,
        runtime_endpoint_state: RuntimeEndpointState,
        state: ParserReadState,
        elapsed_micros: u64,
    ) -> Result<Self, String> {
        if state == ParserReadState::Ready {
            return Err("ready parser authority requires committed generation evidence".to_owned());
        }
        context.validate()?;
        if let Some(generation) = generation {
            generation.validate()?;
            if generation.context != context {
                return Err("generation context differs from parser read context".to_owned());
            }
        }
        Ok(Self {
            schema_id: PARSER_READ_AUTHORITY_SCHEMA_ID.to_owned(),
            schema_version: PARSER_READ_AUTHORITY_SCHEMA_VERSION.to_owned(),
            project_id: context.project_id,
            workspace_id: context.workspace_id,
            canonical_workspace_root: context.canonical_workspace_root,
            language_id: context.language_id,
            provider_id: context.provider_id,
            provider_manifest_digest: context.provider_manifest_digest,
            generation_epoch: generation.map(|value| value.generation_epoch),
            generation_digest: generation.map(|value| value.generation_digest.clone()),
            source_root_digest: generation.map(|value| value.source_root_digest.clone()),
            route: ParserReadRoute::None,
            state,
            runtime_endpoint_state,
            elapsed_micros,
            counters: ParserReadCounters::default(),
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != PARSER_READ_AUTHORITY_SCHEMA_ID
            || self.schema_version != PARSER_READ_AUTHORITY_SCHEMA_VERSION
        {
            return Err("parser read authority schema identity drift".to_owned());
        }
        let context = ParserReadContext {
            project_id: self.project_id.clone(),
            workspace_id: self.workspace_id.clone(),
            canonical_workspace_root: self.canonical_workspace_root.clone(),
            language_id: self.language_id.clone(),
            provider_id: self.provider_id.clone(),
            provider_manifest_digest: self.provider_manifest_digest.clone(),
        };
        context.validate()?;
        let generation =
            match (
                self.generation_epoch,
                &self.generation_digest,
                &self.source_root_digest,
            ) {
                (Some(generation_epoch), Some(generation_digest), Some(source_root_digest)) => {
                    Some(CommittedParserGeneration {
                        context: context.clone(),
                        generation_epoch,
                        generation_digest: generation_digest.clone(),
                        source_root_digest: source_root_digest.clone(),
                    })
                }
                (None, None, None) => None,
                _ => return Err(
                    "parser read authority must carry epoch and both generation digests or none"
                        .to_owned(),
                ),
            };
        if let Some(generation) = &generation {
            generation.validate()?;
        }
        match self.state {
            ParserReadState::Ready => {
                if self.route == ParserReadRoute::None {
                    return Err("ready parser read authority requires a read route".to_owned());
                }
                generation
                    .ok_or_else(|| "ready authority omitted generation evidence".to_owned())?;
                self.counters.validate_read_only()?;
            }
            ParserReadState::GenerationRequired
            | ParserReadState::GenerationStale
            | ParserReadState::GenerationCorrupt => {
                if self.route != ParserReadRoute::None {
                    return Err(
                        "unavailable parser read authority cannot expose a read route".to_owned(),
                    );
                }
            }
        }
        Ok(())
    }
}

fn validate_digest(field: &str, value: &str) -> Result<(), String> {
    let Some((algorithm, digest)) = value.split_once(':') else {
        return Err(format!("{field} must include a digest algorithm"));
    };
    if !matches!(algorithm, "blake3" | "blake3-256" | "sha256")
        || !(32..=64).contains(&digest.len())
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("{field} must be a canonical lowercase digest"));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ParserWorkspaceKey {
    project_id: String,
    workspace_id: String,
    canonical_workspace_root: String,
    language_id: String,
    provider_id: String,
    provider_manifest_digest: String,
}

impl From<&ParserReadContext> for ParserWorkspaceKey {
    fn from(context: &ParserReadContext) -> Self {
        Self {
            project_id: context.project_id.clone(),
            workspace_id: context.workspace_id.clone(),
            canonical_workspace_root: context.canonical_workspace_root.clone(),
            language_id: context.language_id.clone(),
            provider_id: context.provider_id.clone(),
            provider_manifest_digest: context.provider_manifest_digest.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParserReadRequest {
    generation: Arc<CommittedParserGeneration>,
    workspace_key: Arc<ParserWorkspaceKey>,
}

impl ParserReadRequest {
    pub fn new(generation: CommittedParserGeneration) -> Result<Self, String> {
        generation.validate()?;
        let workspace_key = ParserWorkspaceKey::from(&generation.context);
        Ok(Self {
            generation: Arc::new(generation),
            workspace_key: Arc::new(workspace_key),
        })
    }

    pub fn generation(&self) -> &CommittedParserGeneration {
        &self.generation
    }
}

#[derive(Clone, Debug)]
pub struct LoadedParserGeneration {
    pub generation: CommittedParserGeneration,
    pub checkpoint_bytes: Arc<[u8]>,
    pub counters: ParserReadCounters,
}

impl LoadedParserGeneration {
    fn validate_for(&self, expected: &CommittedParserGeneration) -> Result<(), String> {
        self.generation.validate()?;
        if &self.generation != expected {
            return Err(
                "loaded parser generation differs from requested immutable identity".to_owned(),
            );
        }
        if self.checkpoint_bytes.is_empty() {
            return Err("loaded parser generation omitted immutable checkpoint bytes".to_owned());
        }
        self.counters.validate_read_only()?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ParserReadLease {
    pub authority: ParserReadAuthority,
    pub generation: Arc<LoadedParserGeneration>,
}

#[derive(Debug)]
struct ResidentParserGeneration {
    loader_ticket: u64,
    loaded: Arc<LoadedParserGeneration>,
}

type ParserGenerationCell = OnceCell<Arc<ResidentParserGeneration>>;

#[derive(Debug)]
struct ParserWorkspaceEntry {
    generation: Arc<CommittedParserGeneration>,
    cell: Arc<ParserGenerationCell>,
}

#[derive(Clone, Default)]
pub struct ParserReadAuthorityRegistry {
    entries: Arc<RwLock<HashMap<ParserWorkspaceKey, ParserWorkspaceEntry>>>,
    next_ticket: Arc<AtomicU64>,
}

impl ParserReadAuthorityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    async fn generation_cell(
        &self,
        key: &ParserWorkspaceKey,
        expected: &Arc<CommittedParserGeneration>,
    ) -> Result<Arc<ParserGenerationCell>, String> {
        let resident = {
            let entries = self.entries.read().await;
            select_parser_generation_cell(entries.get(key), expected)?
        };
        if let Some(cell) = resident {
            return Ok(cell);
        }
        let mut entries = self.entries.write().await;
        if let Some(cell) = select_parser_generation_cell(entries.get(key), expected)? {
            return Ok(cell);
        }
        let cell = Arc::new(ParserGenerationCell::new());
        entries.insert(
            key.clone(),
            ParserWorkspaceEntry {
                generation: expected.clone(),
                cell: cell.clone(),
            },
        );
        Ok(cell)
    }

    async fn load_generation<F, Fut>(
        cell: Arc<ParserGenerationCell>,
        expected: Arc<CommittedParserGeneration>,
        ticket: u64,
        load: F,
    ) -> Result<Arc<ResidentParserGeneration>, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<LoadedParserGeneration, String>>,
    {
        cell.get_or_try_init(|| async move {
            let loaded = load().await?;
            loaded.validate_for(&expected)?;
            Ok::<Arc<ResidentParserGeneration>, String>(Arc::new(ResidentParserGeneration {
                loader_ticket: ticket,
                loaded: Arc::new(loaded),
            }))
        })
        .await
        .cloned()
    }

    pub async fn acquire<F, Fut>(
        &self,
        request: ParserReadRequest,
        runtime_endpoint_state: RuntimeEndpointState,
        load: F,
    ) -> Result<ParserReadLease, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<LoadedParserGeneration, String>>,
    {
        let started = tokio::time::Instant::now();
        let ticket = self.next_ticket.fetch_add(1, Ordering::Relaxed);
        let expected = request.generation;
        let key = request.workspace_key;
        let cell = self.generation_cell(key.as_ref(), &expected).await?;
        let resident = Self::load_generation(cell, expected.clone(), ticket, load).await?;
        let performed_load = resident.loader_ticket == ticket;
        let authority = ParserReadAuthority::ready(
            expected.as_ref().clone(),
            runtime_endpoint_state,
            !performed_load,
            elapsed_micros(started.elapsed()),
            if performed_load {
                resident.loaded.counters
            } else {
                ParserReadCounters::default()
            },
        )?;
        Ok(ParserReadLease {
            authority,
            generation: resident.loaded.clone(),
        })
    }

    pub async fn resident_generation_count(&self) -> usize {
        let entries = self.entries.read().await;
        entries
            .values()
            .filter(|entry| entry.cell.get().is_some())
            .count()
    }
}

fn select_parser_generation_cell(
    entry: Option<&ParserWorkspaceEntry>,
    expected: &CommittedParserGeneration,
) -> Result<Option<Arc<ParserGenerationCell>>, String> {
    match entry {
        Some(entry) if entry.generation.as_ref() == expected => Ok(Some(entry.cell.clone())),
        Some(entry) if entry.generation.generation_epoch > expected.generation_epoch => {
            Err("requested parser generation is older than the resident epoch".to_owned())
        }
        Some(entry) if entry.generation.generation_epoch == expected.generation_epoch => {
            Err("parser generation digest drift at the same epoch".to_owned())
        }
        Some(_) | None => Ok(None),
    }
}

fn elapsed_micros(elapsed: std::time::Duration) -> u64 {
    u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "../tests/unit/parser_read_authority.rs"]
mod parser_read_authority_tests;
