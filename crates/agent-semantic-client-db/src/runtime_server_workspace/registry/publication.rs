// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed publication commands submitted to a resident workspace writer.

use std::path::PathBuf;

use tokio::sync::oneshot;

use super::{RuntimeServerWorkspaceRegistry, WorkspaceWriteCommand};
use crate::runtime_server_workspace::{
    WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot, WorkspaceRecoveryReceipt,
    WorkspaceRecoverySource, WorkspaceRuntimeSelectorOverlay,
    WorkspaceRuntimeSelectorOverlayReceipt,
};

impl RuntimeServerWorkspaceRegistry {
    /// Acquire exactly the owner paths named by an authenticated Host receipt
    /// and publish one content/index transaction. No workspace walk or
    /// generation rebuild is permitted on this path.
    pub async fn publish_owner_content_mutation_from_paths(
        &self,
        mutation_id: impl Into<String>,
        workspace_identity: &str,
        project_root: &std::path::Path,
        changed_paths: &[std::path::PathBuf],
    ) -> Result<
        Option<crate::runtime_server_workspace::WorkspaceOwnerContentMutationReceiptV1>,
        String,
    > {
        let mutation_id = mutation_id.into();
        if mutation_id.trim().is_empty() {
            return Err("workspace owner content mutation id must be non-empty".to_owned());
        }
        let lease = self.lease(workspace_identity, project_root)?;
        let base_generation_digest = lease.generation().generation_digest.clone();
        let mut owner_paths = std::collections::BTreeSet::new();
        for changed_path in changed_paths {
            let relative = if changed_path.is_absolute() {
                changed_path.strip_prefix(project_root).map_err(|_| {
                    format!(
                        "workspace owner content mutation path is outside project root: {}",
                        changed_path.display()
                    )
                })?
            } else {
                changed_path.as_path()
            };
            let owner_path = relative.to_string_lossy().replace('\\', "/");
            super::super::owner_content_identity::normalized_owner_path(&owner_path)?;
            owner_paths.insert(owner_path);
        }
        let mut upserts = Vec::new();
        let mut removals = Vec::new();
        for owner_path in owner_paths {
            let previous = lease
                .runtime_owner_snapshot(&owner_path)
                .map(|(_, owner)| owner);
            match super::super::owner_content_identity::read(project_root, &owner_path).await? {
                Some(content) => {
                    if previous
                        .as_ref()
                        .is_some_and(|owner| owner.content_digest == content.digest)
                    {
                        continue;
                    }
                    upserts.push(
                        crate::runtime_server_workspace::WorkspaceOwnerContentUpsertV1 {
                            previous_content_digest: previous
                                .as_ref()
                                .map(|owner| owner.content_digest.clone()),
                            owner: WorkspaceOwnerSnapshot {
                                owner_path,
                                authority: previous.and_then(|owner| owner.authority),
                                content_digest: content.digest,
                                native_syntax_diagnostic: None,
                                bytes: content.bytes,
                                selectors: Vec::new(),
                            },
                        },
                    );
                }
                None => {
                    if let Some(previous) = previous {
                        removals.push(
                            crate::runtime_server_workspace::WorkspaceOwnerContentRemovalV1 {
                                owner_path,
                                previous_content_digest: previous.content_digest,
                            },
                        );
                    }
                }
            }
        }
        if upserts.is_empty() && removals.is_empty() {
            return Ok(None);
        }
        self.publish_owner_content_mutation(
            workspace_identity,
            project_root,
            crate::runtime_server_workspace::WorkspaceOwnerContentMutationV1 {
                schema_id:
                    crate::runtime_server_workspace::WORKSPACE_OWNER_CONTENT_MUTATION_SCHEMA_ID
                        .to_owned(),
                schema_version: "1".to_owned(),
                mutation_id,
                base_generation_digest,
                upserts,
                removals,
            },
        )
        .await
        .map(Some)
    }

    pub async fn publish_owner_content_mutation(
        &self,
        workspace_identity: impl Into<String>,
        project_root: &std::path::Path,
        mutation: crate::runtime_server_workspace::WorkspaceOwnerContentMutationV1,
    ) -> Result<crate::runtime_server_workspace::WorkspaceOwnerContentMutationReceiptV1, String>
    {
        let workspace_identity = workspace_identity.into();
        let (mutation, prepared_search_delta) = tokio::task::spawn_blocking(move || {
            let prepared =
                super::super::resident_overlay::prepare_owner_content_search_delta(&mutation)?;
            Ok::<_, String>((mutation, prepared))
        })
        .await
        .map_err(|error| format!("owner skeleton-index preparation task failed: {error}"))??;
        let entry = self.entry(&workspace_identity, project_root).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::PublishOwnerContentMutation(
                super::writer_publication::PublishOwnerContentMutationCommand {
                    target: entry.write_target(),
                    workspace_identity,
                    mutation,
                    prepared_search_delta,
                    reply,
                },
            ))
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive.await.map_err(|_| {
            "runtime workspace writer lane dropped owner content mutation receipt".to_owned()
        })?
    }

    pub async fn publish_selector_overlay(
        &self,
        workspace_identity: impl Into<String>,
        project_root: &std::path::Path,
        overlay: WorkspaceRuntimeSelectorOverlay,
    ) -> Result<WorkspaceRuntimeSelectorOverlayReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity, project_root).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::PublishSelectorOverlay {
                target: entry.write_target(),
                workspace_identity: workspace_identity.clone(),
                overlay,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive.await.map_err(|_| {
            "runtime workspace writer lane dropped selector overlay receipt".to_owned()
        })?
    }

    pub async fn rebind_selector_overlay(
        &self,
        workspace_identity: impl Into<String>,
        project_root: &std::path::Path,
        rebind: WorkspaceRuntimeSelectorRebind,
    ) -> Result<WorkspaceRuntimeSelectorOverlayReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity, project_root).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::RebindSelectorOverlay {
                target: entry.write_target(),
                workspace_identity: workspace_identity.clone(),
                rebind,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive.await.map_err(|_| {
            "runtime workspace writer lane dropped selector rebind receipt".to_owned()
        })?
    }

    pub async fn publish_owner_identity_delta(
        &self,
        source_mutation_id: impl Into<String>,
        workspace_identity: &str,
        project_root: &std::path::Path,
        changed_paths: &[std::path::PathBuf],
    ) -> Result<(), String> {
        let source_mutation_id = source_mutation_id.into();
        if source_mutation_id.trim().is_empty() {
            return Err("runtime owner identity mutation id must be non-empty".to_owned());
        }
        let entry = self.entry(workspace_identity, project_root).await?;
        let mut delta = Vec::with_capacity(changed_paths.len());
        for changed_path in changed_paths {
            let relative = changed_path.strip_prefix(project_root).map_err(|_| {
                format!(
                    "runtime owner identity path is outside project root: {}",
                    changed_path.display()
                )
            })?;
            let owner_path = relative.to_string_lossy().replace('\\', "/");
            let content =
                super::super::owner_content_identity::read(project_root, &owner_path).await?;
            delta.push(match content {
                Some(content) => super::super::owner_identity_journal::RuntimeOwnerIdentityEntry {
                    owner_path,
                    state: super::super::owner_identity_journal::RuntimeOwnerIdentityState::Present,
                    content_digest: Some(content.digest),
                    mutation_id: None,
                },
                None => super::super::owner_identity_journal::RuntimeOwnerIdentityEntry {
                    owner_path,
                    state: super::super::owner_identity_journal::RuntimeOwnerIdentityState::Missing,
                    content_digest: None,
                    mutation_id: None,
                },
            });
        }
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::PublishOwnerIdentityDelta {
                target: entry.write_target(),
                source_mutation_id,
                workspace_identity: workspace_identity.to_owned(),
                delta,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped identity receipt".to_owned())?
    }

    pub async fn publish(
        &self,
        request_id: impl Into<String>,
        source: WorkspaceRecoverySource,
        generation: WorkspaceMemoryGeneration,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        generation.validate()?;
        let entry = self
            .entry(
                &generation.workspace_identity,
                std::path::Path::new(&generation.project_root),
            )
            .await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::Publish {
                target: entry.write_target(),
                request_id: request_id.into(),
                source,
                generation,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }

    pub async fn publish_provider_owner(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: &std::path::Path,
        owner: WorkspaceOwnerSnapshot,
    ) -> Result<(), String> {
        let request_id = request_id.into();
        let workspace_identity = workspace_identity.into();
        if self
            .ready_entry(&workspace_identity, project_root)?
            .is_some()
        {
            self.publish_owner_overlay(request_id, workspace_identity, project_root, owner)
                .await?;
        } else {
            self.sparse_provider_owners
                .publish(&workspace_identity, project_root, owner)?;
        }
        Ok(())
    }

    pub async fn publish_owner_overlay(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: &std::path::Path,
        owner: WorkspaceOwnerSnapshot,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity, project_root).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::PublishOwnerOverlay {
                target: entry.write_target(),
                request_id: request_id.into(),
                workspace_identity,
                owner,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }

    /// Atomically attach parser-owned selectors and relations to the current
    /// resident generation without minting or durably rewriting a full source
    /// generation. The content-addressed parser artifact is the restart
    /// authority; this overlay is the request-serving read model.
    pub async fn publish_resident_owner_symbol_rebind(
        &self,
        workspace_identity: impl Into<String>,
        project_root: &std::path::Path,
        rebind: crate::runtime_server_workspace::WorkspaceOwnerSymbolRebindV1,
    ) -> Result<crate::runtime_server_workspace::WorkspaceOwnerSymbolRebindReceiptV1, String> {
        let workspace_identity = workspace_identity.into();
        let prepare_input = rebind.clone();
        let prepared = tokio::task::spawn_blocking(move || {
            super::super::resident_overlay::prepare_owner_symbol_rebind(&prepare_input)
        })
        .await
        .map_err(|error| format!("owner symbol rebind preparation task failed: {error}"))??;
        let entry = self.entry(&workspace_identity, project_root).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::PublishResidentOwnerSymbolRebind(
                super::writer_publication::PublishResidentOwnerSymbolRebindCommand {
                    target: entry.write_target(),
                    workspace_identity,
                    rebind,
                    prepared,
                    reply,
                },
            ))
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }

    pub async fn tombstone_owner_overlay(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: &std::path::Path,
        owner_path: impl Into<String>,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity, project_root).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::TombstoneOwnerOverlay {
                target: entry.write_target(),
                request_id: request_id.into(),
                workspace_identity,
                owner_path: owner_path.into(),
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }

    pub async fn relocate_owner_overlay(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: &std::path::Path,
        previous_owner_path: impl Into<String>,
        owner: WorkspaceOwnerSnapshot,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity, project_root).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::RelocateOwnerOverlay {
                target: entry.write_target(),
                request_id: request_id.into(),
                workspace_identity,
                previous_owner_path: previous_owner_path.into(),
                owner,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }

    pub async fn admit_canonical_generation_resident(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        materialization: crate::runtime_server_workspace::ValidatedWorkspaceCanonicalMaterialization,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        self.admit_canonical_generation_resident_inner(
            request_id.into(),
            workspace_identity.into(),
            materialization,
            None,
        )
        .await
    }

    pub(crate) async fn admit_canonical_generation_resident_with_candidate(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        materialization: crate::runtime_server_workspace::ValidatedWorkspaceCanonicalMaterialization,
        admission_candidate: crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        self.admit_canonical_generation_resident_inner(
            request_id.into(),
            workspace_identity.into(),
            materialization,
            Some(admission_candidate),
        )
        .await
    }

    async fn admit_canonical_generation_resident_inner(
        &self,
        request_id: String,
        workspace_identity: String,
        materialization: crate::runtime_server_workspace::ValidatedWorkspaceCanonicalMaterialization,
        admission_candidate: Option<
            crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
        >,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let project_root = std::path::Path::new(&materialization.as_materialization().project_root);
        let active = match self.ready_entry(&workspace_identity, project_root) {
            Ok(Some(entry)) => entry.current.borrow().clone(),
            Ok(None) | Err(_) => None,
        };
        let active_pointer_matches = if let Some(active) = active.as_ref() {
            match super::super::workspace_generation_pointer_path(
                &self.root,
                &workspace_identity,
                project_root,
            ) {
                Ok(pointer_path) => {
                    super::super::WorkspaceGenerationPointerReader::matches_generation(
                        &pointer_path,
                        active.generation(),
                    )
                    .await
                }
                Err(_) => false,
            }
        } else {
            false
        };
        if let Some(active) = active
            && active.generation().workspace_identity
                == materialization.as_materialization().workspace_identity
            && active
                .generation()
                .source_snapshot
                .has_same_content_identity(&materialization.as_materialization().source_snapshot)
            && active.generation().provider_schema_digest
                == materialization.as_materialization().provider_schema_digest
            && active.generation().runtime_provider_execution_binding
                == materialization
                    .as_materialization()
                    .runtime_provider_execution_binding
            && active.generation().module_graph_digest
                == materialization.as_materialization().import_digest
            && active.generation().selector_set_digest
                == materialization.as_materialization().selector_set_digest
            && active.generation().workspace_source_scope_generation
                == materialization
                    .as_materialization()
                    .workspace_source_scope_generation
            && active_pointer_matches
            && matches!(
                self.published_generation_state(&workspace_identity, project_root)
                    .await?,
                crate::runtime_server_workspace::PublishedWorkspaceGenerationState::Ready
            )
        {
            let target_epoch = active.generation().active_epoch;
            let receipt = crate::runtime_server_workspace::WorkspaceRecoveryReceipt {
                projection_capability: active
                    .generation()
                    .projection_capability_receipt(active.generation().active_epoch)?,
                schema_id: crate::runtime_server_workspace::WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID
                    .to_owned(),
                schema_version: "1".to_owned(),
                request_id,
                workspace_identity: workspace_identity.clone(),
                source: WorkspaceRecoverySource::MmapCheckpoint,
                state: crate::runtime_server_workspace::WorkspaceGenerationState::Ready,
                active_epoch: target_epoch.saturating_sub(1),
                target_epoch,
                generation_digest: active.generation().generation_digest.clone(),
                source_root_digest: active.generation().source_snapshot.root_digest.clone(),
                old_generation_readable: target_epoch > 1,
                resident_publication_elapsed_micros: 0,
                counters: crate::runtime_server_workspace::RuntimeDataPlaneCounters::default(),
            };
            receipt.validate()?;
            self.sparse_provider_owners
                .evict_scope(&workspace_identity, project_root);
            return Ok(receipt);
        }
        let (materialization, prepared_index) = materialization.into_parts();
        let project_root = std::path::PathBuf::from(&materialization.project_root);
        let entry = self
            .replacement_entry(&workspace_identity, &project_root)
            .await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::EnsureCanonicalGeneration(
                super::canonical_publication_owner::EnsureCanonicalGenerationCommand {
                    target: entry.write_target(),
                    request_id,
                    workspace_identity: workspace_identity.clone(),
                    materialization,
                    prepared_index,
                    admission_candidate,
                    persisted_epoch: entry.persisted_epoch,
                    reply,
                },
            ))
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        let receipt = receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its completion".to_owned())??;
        self.sparse_provider_owners
            .evict_scope(&workspace_identity, &project_root);
        Ok(receipt)
    }

    pub async fn ensure_canonical_generation(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        materialization: crate::runtime_server_workspace::ValidatedWorkspaceCanonicalMaterialization,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let project_root =
            std::path::PathBuf::from(&materialization.as_materialization().project_root);
        let receipt = self
            .admit_canonical_generation_resident(
                request_id,
                workspace_identity.clone(),
                materialization,
            )
            .await?;
        self.wait_canonical_generation_durable(
            &workspace_identity,
            &project_root,
            &receipt.generation_digest,
            receipt.target_epoch,
        )
        .await?;
        Ok(receipt)
    }

    pub async fn wait_canonical_generation_durable(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
        generation_digest: &str,
        target_epoch: u64,
    ) -> Result<(), String> {
        self.wait_canonical_generation_durable_inner(
            workspace_identity,
            project_root,
            generation_digest,
            target_epoch,
        )
        .await
    }

    async fn wait_canonical_generation_durable_inner(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
        generation_digest: &str,
        target_epoch: u64,
    ) -> Result<(), String> {
        let entry = self
            .ready_entry(workspace_identity, project_root)?
            .ok_or_else(|| {
                format!(
                    "runtime workspace entry is unavailable while waiting for canonical durability: workspaceIdentity={workspace_identity}"
                )
            })?;
        let mut durability = entry.durability.subscribe();
        loop {
            let receipt = durability.borrow().clone();
            if let Some(receipt) = receipt {
                receipt.validate()?;
                if receipt.workspace_identity == workspace_identity
                    && receipt.generation_digest == generation_digest
                    && receipt.target_epoch == target_epoch
                {
                    match receipt.state {
                        crate::runtime_server_workspace::WorkspaceGenerationDurabilityState::DurableReady => {
                            return Ok(());
                        }
                        crate::runtime_server_workspace::WorkspaceGenerationDurabilityState::Failed => {
                            return Err(receipt.failure.unwrap_or_else(|| {
                                "canonical workspace generation durability failed without a diagnostic"
                                    .to_owned()
                            }));
                        }
                        crate::runtime_server_workspace::WorkspaceGenerationDurabilityState::ResidentReady => {}
                    }
                } else if receipt.workspace_identity == workspace_identity
                    && receipt.target_epoch > target_epoch
                {
                    return Err(format!(
                        "canonical workspace generation was superseded before durability: workspaceIdentity={workspace_identity} expectedEpoch={target_epoch} actualEpoch={} expectedDigest={generation_digest} actualDigest={}",
                        receipt.target_epoch, receipt.generation_digest
                    ));
                }
            }
            durability.changed().await.map_err(|_| {
                "runtime workspace generation durability receipt channel closed".to_owned()
            })?;
        }
    }

    pub async fn restore_checkpoint(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: &std::path::Path,
        path: PathBuf,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity, project_root).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::RestoreCheckpoint {
                target: entry.write_target(),
                request_id: request_id.into(),
                workspace_identity,
                path,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }
}

use crate::runtime_server_workspace::WorkspaceRuntimeSelectorRebind;
