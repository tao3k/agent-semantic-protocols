// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Memory-first ready admission for an already resident workspace generation.

use std::path::Path;

use super::model::WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID;
use super::{
    RuntimeDataPlaneCounters, RuntimeServerWorkspaceRegistry, WorkspaceGenerationState,
    WorkspaceRecoveryReceipt, WorkspaceRecoverySource,
};

impl RuntimeServerWorkspaceRegistry {
    /// Admit the immutable pointer for Search/Query without decoding the
    /// canonical generation into the mutable writer registry.
    ///
    /// The caller must first establish the exact V1 admission binding against
    /// the current repository candidate and Runtime bundle. Projection mmap
    /// corruption remains fail-closed in the query-generation observer, which
    /// opens the exact and Search segments before publishing Ready.
    pub async fn restore_published_query_generation(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: &Path,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let pointer_path = super::workspace_generation_pointer_path(
            &self.root,
            &workspace_identity,
            project_root,
        )?;
        let pointer = super::WorkspaceGenerationPointerReader::open(&pointer_path).await?;
        let snapshot = pointer.read()?;
        snapshot.validate()?;
        if snapshot.workspace_identity != workspace_identity {
            return Err("durable query generation workspace identity drift".to_owned());
        }
        let execution_binding = snapshot
            .runtime_provider_execution_binding
            .as_ref()
            .ok_or_else(|| {
                "durable query generation lacks Runtime provider execution binding".to_owned()
            })?;
        execution_binding.validate()?;
        let projection_capability =
            crate::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest {
                provider_catalog_digest: execution_binding.workspace_closure_digest.clone(),
                selectors: Vec::new(),
            }
            .into_ready_receipt(
                workspace_identity.clone(),
                snapshot.generation_digest.clone(),
                snapshot.source_root_digest.clone(),
                snapshot.active_epoch,
            )?;
        let receipt = WorkspaceRecoveryReceipt {
            schema_id: WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            request_id: request_id.into(),
            workspace_identity: workspace_identity.clone(),
            source: WorkspaceRecoverySource::MmapCheckpoint,
            state: WorkspaceGenerationState::Ready,
            active_epoch: snapshot.active_epoch.saturating_sub(1),
            target_epoch: snapshot.active_epoch,
            generation_digest: snapshot.generation_digest,
            source_root_digest: snapshot.source_root_digest,
            projection_capability,
            old_generation_readable: snapshot.previous_epoch_readable,
            resident_publication_elapsed_micros: 0,
            counters: RuntimeDataPlaneCounters::default(),
        };
        receipt.validate()?;
        self.publish_durable_query_binding(
            workspace_identity,
            project_root.to_path_buf(),
            super::registry::DurableQueryBinding {
                generation_digest: receipt.generation_digest.clone(),
                runtime_bundle_digest: execution_binding.runtime_bundle_digest.clone(),
            },
        );
        Ok(receipt)
    }

    pub async fn published_generation_state(
        &self,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Result<super::PublishedWorkspaceGenerationState, String> {
        self.published_generation_state_inner(workspace_identity, project_root)
            .await
    }

    async fn published_generation_state_inner(
        &self,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Result<super::PublishedWorkspaceGenerationState, String> {
        let pointer_path =
            super::workspace_generation_pointer_path(&self.root, workspace_identity, project_root)?;
        match super::WorkspaceGenerationDataPlaneClient::open_state(&pointer_path).await? {
            super::WorkspaceGenerationDataPlaneOpen::Ready(_client) => {
                // Ready is a property of the complete immutable generation,
                // not merely its owner/source segment. In particular, an old
                // exact-projection mmap must force Runtime-owned republication
                // instead of letting the query path fail after admission.
                match super::WorkspaceExactProjectionDataPlaneClient::open_state(&pointer_path)
                    .await?
                {
                    super::WorkspaceExactProjectionDataPlaneOpen::Ready(_client) => {
                        match super::WorkspaceSearchGenerationDataPlaneClient::open(
                            &pointer_path,
                            project_root,
                        )
                        .await
                        {
                            Ok(client) => {
                                let workspace_generation = &client.authority().workspace_generation;
                                if workspace_generation.owner_count == 0
                                    && workspace_generation.leaf_count == 0
                                {
                                    return Ok(
                            super::PublishedWorkspaceGenerationState::RecoveryRequired {
                                reason: "provider-owner-inventory-empty: published generation has zero owners and zero leaves"
                                    .to_owned(),
                            },
                        );
                                }
                                Ok(super::PublishedWorkspaceGenerationState::Ready)
                            }
                            Err(reason) => {
                                Ok(super::PublishedWorkspaceGenerationState::RecoveryRequired {
                                    reason,
                                })
                            }
                        }
                    }
                    super::WorkspaceExactProjectionDataPlaneOpen::Missing => {
                        Ok(super::PublishedWorkspaceGenerationState::Missing)
                    }
                    super::WorkspaceExactProjectionDataPlaneOpen::RecoveryRequired { reason } => {
                        Ok(super::PublishedWorkspaceGenerationState::RecoveryRequired { reason })
                    }
                }
            }
            super::WorkspaceGenerationDataPlaneOpen::Missing => {
                Ok(super::PublishedWorkspaceGenerationState::Missing)
            }
            super::WorkspaceGenerationDataPlaneOpen::RecoveryRequired { reason } => {
                Ok(super::PublishedWorkspaceGenerationState::RecoveryRequired { reason })
            }
        }
    }

    pub async fn restore_published_generation(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: &Path,
    ) -> Result<super::WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        self.ensure_entry_ready(&workspace_identity, project_root)
            .await?;
        self.ready_recovery_receipt(request_id, &workspace_identity, project_root)
    }

    /// Returns a typed ready receipt without reopening durable state when the generation is resident.
    pub fn ready_recovery_receipt(
        &self,
        request_id: impl Into<String>,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let lease = self.lease(workspace_identity, project_root)?;
        let generation = lease.generation();
        let target_epoch = generation.active_epoch;
        let active_epoch = target_epoch
            .checked_sub(1)
            .ok_or_else(|| "runtime workspace generation epoch must be positive".to_owned())?;
        let receipt = WorkspaceRecoveryReceipt {
            projection_capability: generation
                .projection_capability_receipt(generation.active_epoch)?,
            schema_id: WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            request_id: request_id.into(),
            workspace_identity: workspace_identity.to_owned(),
            source: WorkspaceRecoverySource::MmapCheckpoint,
            state: WorkspaceGenerationState::Ready,
            active_epoch,
            target_epoch,
            generation_digest: generation.generation_digest.clone(),
            source_root_digest: generation.source_snapshot.root_digest.clone(),
            old_generation_readable: active_epoch != 0,
            resident_publication_elapsed_micros: 0,
            counters: RuntimeDataPlaneCounters::default(),
        };
        receipt.validate()?;
        Ok(receipt)
    }
}
