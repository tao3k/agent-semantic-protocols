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

    pub async fn publish_owner_delta(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: &std::path::Path,
        delta: crate::runtime_server_workspace::WorkspaceGenerationDelta,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity, project_root).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::PublishOwnerDelta(
                super::writer_publication::PublishOwnerDeltaCommand {
                    target: entry.write_target(),
                    request_id: request_id.into(),
                    workspace_identity,
                    delta,
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
        )
        .await
    }

    async fn admit_canonical_generation_resident_inner(
        &self,
        request_id: String,
        workspace_identity: String,
        materialization: crate::runtime_server_workspace::ValidatedWorkspaceCanonicalMaterialization,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let project_root = std::path::Path::new(&materialization.as_materialization().project_root);
        let active = match self.ready_entry(&workspace_identity, project_root) {
            Ok(Some(entry)) => entry.current.borrow().clone(),
            Ok(None) | Err(_) => None,
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
            && active.generation().module_graph_digest
                == materialization.as_materialization().import_digest
            && active.generation().selector_set_digest
                == materialization.as_materialization().selector_set_digest
            && active.generation().workspace_source_scope_generation
                == materialization
                    .as_materialization()
                    .workspace_source_scope_generation
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
            self.prepare_search_projection_client(&workspace_identity, project_root)
                .await?;
            self.sparse_provider_owners
                .evict_scope(&workspace_identity, project_root);
            return Ok(receipt);
        }
        let (materialization, prepared_index) = materialization.into_parts();
        let project_root = std::path::PathBuf::from(&materialization.project_root);
        let entry = self.entry(&workspace_identity, &project_root).await?;
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
                    reply,
                },
            ))
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        let receipt = receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its completion".to_owned())??;
        self.wait_canonical_generation_durable(
            &workspace_identity,
            &project_root,
            &receipt.generation_digest,
            receipt.target_epoch,
        )
        .await?;
        self.prepare_search_projection_client(&workspace_identity, &project_root)
            .await?;
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
