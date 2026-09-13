// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Candidate-scoped parser materialization outside SearchCoreReady.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures_util::{StreamExt, stream};

use crate::runtime_asp_client::AspClientOperationError;

#[derive(Clone, Default)]
pub(crate) struct RuntimeOwnerMaterializer {
    claims: MaterializationClaims,
}

type MaterializationClaims = Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>;

struct ProviderProjectionGroup {
    projected: Vec<agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerProjection>,
    timing: ProviderProjectionTiming,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ProviderProjectionTiming {
    cache_probe_micros: u128,
    provider_start_micros: u128,
    provider_ready_micros: u128,
    projection_micros: u128,
    request_wall_micros: u128,
}

impl ProviderProjectionTiming {
    fn runtime_engine_work_micros(self) -> u128 {
        self.cache_probe_micros
            .saturating_add(self.projection_micros)
    }

    fn provider_lifecycle_work_micros(self) -> u128 {
        self.provider_start_micros
            .saturating_add(self.provider_ready_micros)
    }
}

struct MaterializationClaim {
    claims: MaterializationClaims,
    key: String,
    mutex: Arc<tokio::sync::Mutex<()>>,
}

impl Drop for MaterializationClaim {
    fn drop(&mut self) {
        let Ok(mut claims) = self.claims.lock() else {
            return;
        };
        if Arc::strong_count(&self.mutex) == 2
            && claims
                .get(&self.key)
                .is_some_and(|current| Arc::ptr_eq(current, &self.mutex))
        {
            claims.remove(&self.key);
        }
    }
}

impl RuntimeOwnerMaterializer {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn ensure_candidates(
        &self,
        _request_id: &str,
        workspace_identity: &str,
        project_root: &Path,
        parser_artifact_root: &Path,
        generation_digest: &str,
        owners: &BTreeSet<String>,
        providers: &[agent_semantic_search::WorkspaceSearchProvider],
        runtime: &agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
        registry: &Arc<
            agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
        >,
    ) -> Result<
        agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
        AspClientOperationError,
    > {
        let total_started = std::time::Instant::now();
        if owners.is_empty() {
            return registry
                .resident_read_client(workspace_identity, project_root)
                .map_err(AspClientOperationError::Message);
        }
        let resident_acquire_started = std::time::Instant::now();
        let resident = registry
            .resident_read_client(workspace_identity, project_root)
            .map_err(AspClientOperationError::Message)?;
        let resident_acquire_micros = resident_acquire_started.elapsed().as_micros();
        // Acquire per-owner claims in canonical order.  Overlapping requests
        // therefore coalesce without deadlock, while disjoint candidate sets
        // may still progress independently.
        let claim_started = std::time::Instant::now();
        let mut claims = Vec::new();
        for owner_path in owners {
            if resident
                .semantic_owner_materialized(owner_path)
                .map_err(AspClientOperationError::Message)?
            {
                continue;
            }
            let owner = resident
                .owner_snapshot(owner_path)
                .map_err(AspClientOperationError::Message)?
                .ok_or_else(|| {
                    AspClientOperationError::Message(format!(
                        "candidate parser owner disappeared: ownerPath={owner_path}"
                    ))
                })?;
            let key = format!(
                "{workspace_identity}\0{generation_digest}\0{owner_path}\0{}",
                owner.content_digest
            );
            let claim = self.claim(&key)?;
            let guard = Arc::clone(&claim.mutex).lock_owned().await;
            claims.push((claim, guard));
        }
        let claim_micros = claim_started.elapsed().as_micros();
        let resident_refresh_started = std::time::Instant::now();
        let current = registry
            .resident_read_client(workspace_identity, project_root)
            .map_err(AspClientOperationError::Message)?;
        let resident_refresh_micros = resident_refresh_started.elapsed().as_micros();
        let mut pending_by_language = BTreeMap::<
            String,
            Vec<agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot>,
        >::new();
        let mut expected = BTreeMap::new();
        let mut pending_owner_count = 0usize;
        for owner_path in owners {
            if current
                .semantic_owner_materialized(owner_path)
                .map_err(AspClientOperationError::Message)?
            {
                continue;
            }
            let provider = provider_for_owner(owner_path, providers)?;
            let owner = current
                .owner_snapshot(owner_path)
                .map_err(AspClientOperationError::Message)?
                .ok_or_else(|| {
                    AspClientOperationError::Message(format!(
                        "candidate parser owner disappeared after claim: ownerPath={owner_path}"
                    ))
                })?;
            pending_by_language
                .entry(provider.language_id.clone())
                .or_default()
                .push(owner.clone());
            expected.insert(owner_path.clone(), owner);
            pending_owner_count += 1;
        }
        if pending_by_language.is_empty() {
            let runtime_engine_work_micros = resident_acquire_micros
                .saturating_add(claim_micros)
                .saturating_add(resident_refresh_micros);
            let request_wall_micros = total_started.elapsed().as_micros();
            eprintln!(
                "[runtime-owner-materialization-timing] ownerCount={} providerGroupCount=0 residentAcquireMicros={} claimMicros={} residentRefreshMicros={} providerProjectionWallMicros=0 providerEngineWorkMicros=0 providerLifecycleWorkMicros=0 validationMicros=0 publicationMicros=0 runtimeEngineWorkMicros={} requestWallMicros={} state=resident-hit",
                owners.len(),
                resident_acquire_micros,
                claim_micros,
                resident_refresh_micros,
                runtime_engine_work_micros,
                request_wall_micros,
            );
            return Ok(current);
        }
        let provider_group_count = pending_by_language.len();
        let auxiliary_owners = current
            .auxiliary_owner_snapshots()
            .map_err(AspClientOperationError::Message)?;
        let projection_started = std::time::Instant::now();
        let projected_groups = stream::iter(pending_by_language.into_iter().map(
            |(language_id, owners)| {
                project_provider_group(
                    runtime.clone(),
                    workspace_identity.to_owned(),
                    project_root.to_path_buf(),
                    parser_artifact_root.to_path_buf(),
                    language_id,
                    owners,
                    auxiliary_owners.clone(),
                )
            },
        ))
        .buffer_unordered(providers.len().max(1))
        .collect::<Vec<_>>()
        .await;
        let provider_projection_wall_micros = projection_started.elapsed().as_micros();
        let mut projected = Vec::new();
        let mut provider_engine_work_micros = 0u128;
        let mut provider_lifecycle_work_micros = 0u128;
        for group in projected_groups {
            let group = group?;
            provider_engine_work_micros = provider_engine_work_micros
                .saturating_add(group.timing.runtime_engine_work_micros());
            provider_lifecycle_work_micros = provider_lifecycle_work_micros
                .saturating_add(group.timing.provider_lifecycle_work_micros());
            projected.extend(group.projected);
        }
        let validation_started = std::time::Instant::now();
        let mut seen = BTreeSet::new();
        for projection in &projected {
            let owner_path = projection.owner.owner_path.as_str();
            let owner = expected.get(owner_path).ok_or_else(|| {
                AspClientOperationError::Message(format!(
                    "candidate parser returned an unrequested owner: ownerPath={owner_path}"
                ))
            })?;
            if !seen.insert(owner_path) {
                return Err(AspClientOperationError::Message(format!(
                    "candidate parser returned a duplicate owner: ownerPath={owner_path}"
                )));
            }
            if projection.owner.content_digest != owner.content_digest
                || projection.owner.bytes != owner.bytes
            {
                return Err(AspClientOperationError::Message(format!(
                    "candidate parser product escaped immutable source identity: ownerPath={owner_path}"
                )));
            }
        }
        if projected.len() != pending_owner_count {
            return Err(AspClientOperationError::Message(
                "candidate parser batch omitted an admitted owner".to_owned(),
            ));
        }
        projected.sort_by(|left, right| left.owner.owner_path.cmp(&right.owner.owner_path));
        let mut delta_owners = Vec::with_capacity(projected.len());
        let mut delta_relations = Vec::new();
        for projection in projected {
            delta_owners.push(projection.owner);
            delta_relations.extend(projection.relations);
        }
        let validation_micros = validation_started.elapsed().as_micros();
        let publication_started = std::time::Instant::now();
        registry
            .publish_resident_owner_delta(
                workspace_identity.to_owned(),
                project_root,
                agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDelta {
                    schema_id: agent_semantic_client_db::runtime_server_workspace::WORKSPACE_GENERATION_DELTA_SCHEMA_ID.to_owned(),
                    schema_version: "2".to_owned(),
                    base_generation_digest: generation_digest.to_owned(),
                    owners: delta_owners,
                    tombstones: Vec::new(),
                    relations: delta_relations,
                },
            )
            .await
            .map_err(AspClientOperationError::Message)?;
        let publication_micros = publication_started.elapsed().as_micros();
        drop(claims);
        let runtime_engine_work_micros = resident_acquire_micros
            .saturating_add(claim_micros)
            .saturating_add(resident_refresh_micros)
            .saturating_add(provider_engine_work_micros)
            .saturating_add(validation_micros)
            .saturating_add(publication_micros);
        let request_wall_micros = total_started.elapsed().as_micros();
        eprintln!(
            "[runtime-owner-materialization-timing] ownerCount={} providerGroupCount={} residentAcquireMicros={} claimMicros={} residentRefreshMicros={} providerProjectionWallMicros={} providerEngineWorkMicros={} providerLifecycleWorkMicros={} validationMicros={} publicationMicros={} runtimeEngineWorkMicros={} requestWallMicros={} state=ready",
            owners.len(),
            provider_group_count,
            resident_acquire_micros,
            claim_micros,
            resident_refresh_micros,
            provider_projection_wall_micros,
            provider_engine_work_micros,
            provider_lifecycle_work_micros,
            validation_micros,
            publication_micros,
            runtime_engine_work_micros,
            request_wall_micros,
        );
        registry
            .resident_read_client(workspace_identity, project_root)
            .map_err(AspClientOperationError::Message)
    }

    fn claim(&self, key: &str) -> Result<MaterializationClaim, AspClientOperationError> {
        let mutex = Arc::clone(
            self.claims
                .lock()
                .map_err(|_| {
                    AspClientOperationError::Message(
                        "owner materialization claims poisoned".to_owned(),
                    )
                })?
                .entry(key.to_owned())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))),
        );
        Ok(MaterializationClaim {
            claims: Arc::clone(&self.claims),
            key: key.to_owned(),
            mutex,
        })
    }
}

async fn project_provider_group(
    runtime: agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
    workspace_identity: String,
    project_root: PathBuf,
    parser_artifact_root: PathBuf,
    language_id: String,
    owners: Vec<agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot>,
    auxiliary_owners: Vec<
        agent_semantic_client_db::runtime_server_workspace::WorkspaceAuxiliaryOwnerSnapshot,
    >,
) -> Result<ProviderProjectionGroup, AspClientOperationError> {
    let total_started = std::time::Instant::now();
    let owner_count = owners.len();
    let cache_started = std::time::Instant::now();
    let cached_or_runtime = runtime
        .provider_owners(
            workspace_identity.clone(),
            project_root.clone(),
            parser_artifact_root.clone(),
            language_id.clone(),
            owners.clone(),
            auxiliary_owners.clone(),
        )
        .await;
    match cached_or_runtime {
        Ok(projected) => {
            let timing = ProviderProjectionTiming {
                cache_probe_micros: cache_started.elapsed().as_micros(),
                request_wall_micros: total_started.elapsed().as_micros(),
                ..ProviderProjectionTiming::default()
            };
            eprintln!(
                "[runtime-owner-provider-group-timing] languageId={} ownerCount={} cacheProbeMicros={} providerStartMicros=0 providerReadyMicros=0 providerLifecycleWorkMicros=0 projectionMicros=0 runtimeEngineWorkMicros={} requestWallMicros={} state=cache-hit",
                language_id,
                owner_count,
                timing.cache_probe_micros,
                timing.runtime_engine_work_micros(),
                timing.request_wall_micros,
            );
            Ok(ProviderProjectionGroup { projected, timing })
        }
        Err(error) if error == "state=cache-miss reasonKind=provider-parser-runtime-required" => {
            let cancellation = agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation::new();
            let cache_probe_micros = cache_started.elapsed().as_micros();
            let provider_start_started = std::time::Instant::now();
            runtime
                .provider_runtime(project_root.clone(), language_id.clone())
                .await
                .map_err(AspClientOperationError::Message)?;
            let provider_start_micros = provider_start_started.elapsed().as_micros();
            let provider_ready_started = std::time::Instant::now();
            runtime
                .provider_runtime_await_ready(
                    project_root.clone(),
                    language_id.clone(),
                    cancellation,
                )
                .await
                .map_err(AspClientOperationError::Message)?;
            let provider_ready_micros = provider_ready_started.elapsed().as_micros();
            let projection_started = std::time::Instant::now();
            let projected = runtime
                .provider_owners(
                    workspace_identity,
                    project_root,
                    parser_artifact_root,
                    language_id.clone(),
                    owners,
                    auxiliary_owners,
                )
                .await
                .map_err(AspClientOperationError::Message)?;
            let timing = ProviderProjectionTiming {
                cache_probe_micros,
                provider_start_micros,
                provider_ready_micros,
                projection_micros: projection_started.elapsed().as_micros(),
                request_wall_micros: total_started.elapsed().as_micros(),
            };
            eprintln!(
                "[runtime-owner-provider-group-timing] languageId={} ownerCount={} cacheProbeMicros={} providerStartMicros={} providerReadyMicros={} providerLifecycleWorkMicros={} projectionMicros={} runtimeEngineWorkMicros={} requestWallMicros={} state=ready",
                language_id,
                owner_count,
                timing.cache_probe_micros,
                timing.provider_start_micros,
                timing.provider_ready_micros,
                timing.provider_lifecycle_work_micros(),
                timing.projection_micros,
                timing.runtime_engine_work_micros(),
                timing.request_wall_micros,
            );
            Ok(ProviderProjectionGroup { projected, timing })
        }
        Err(error) => Err(AspClientOperationError::Message(error)),
    }
}

fn provider_for_owner<'a>(
    owner_path: &str,
    providers: &'a [agent_semantic_search::WorkspaceSearchProvider],
) -> Result<&'a agent_semantic_search::WorkspaceSearchProvider, AspClientOperationError> {
    let extension = Path::new(owner_path)
        .extension()
        .and_then(|extension| extension.to_str())
        .ok_or_else(|| {
            AspClientOperationError::Message(format!(
                "candidate parser owner has no admitted extension: ownerPath={owner_path}"
            ))
        })?;
    let mut matches = providers.iter().filter(|provider| {
        provider
            .source_extensions
            .iter()
            .any(|item| item == extension)
    });
    let provider = matches.next().ok_or_else(|| {
        AspClientOperationError::Message(format!(
            "candidate parser owner has no installed provider: ownerPath={owner_path}"
        ))
    })?;
    if matches.next().is_some() {
        return Err(AspClientOperationError::Message(format!(
            "candidate parser owner provider is ambiguous: ownerPath={owner_path}"
        )));
    }
    Ok(provider)
}

#[cfg(test)]
#[path = "../tests/unit/runtime_owner_materialization.rs"]
mod tests;
