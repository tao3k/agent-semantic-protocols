//! Provider-owner operations over the Runtime Server workspace transport.

use crate::{
    ProviderIncrementalOwnerSnapshot, ProviderIncrementalOwnerWrite, ProviderIncrementalScoped,
    ProviderIncrementalWriteReceipt, ProviderOwnerBatchProbeReceipt,
    ProviderOwnerBatchProbeRequest, ProviderOwnerProbe,
};

use super::{WorkspaceDbIpcOperation, WorkspaceDbIpcResult, WorkspaceDbIpcSession};

impl WorkspaceDbIpcSession {
    pub async fn probe_provider_owners(
        &self,
        scope: &ProviderIncrementalScoped,
        owners: &[ProviderOwnerBatchProbeRequest],
    ) -> Result<ProviderOwnerBatchProbeReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ProbeProviderOwners {
                scope: scope.clone(),
                owners: owners.to_vec(),
            })
            .await?
        {
            WorkspaceDbIpcResult::ProviderOwners { receipt } => Ok(receipt),
            _ => Err("workspace owner IPC returned an unexpected probe result".to_owned()),
        }
    }

    pub async fn write_provider_incremental_owner(
        &self,
        request: &ProviderIncrementalOwnerWrite,
    ) -> Result<ProviderIncrementalWriteReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::WriteProviderIncrementalOwner {
                request: request.clone(),
            })
            .await?
        {
            WorkspaceDbIpcResult::ProviderIncrementalOwner { receipt } => Ok(receipt),
            _ => Err(
                "workspace owner IPC returned an unexpected incremental write result".to_owned(),
            ),
        }
    }

    pub async fn read_provider_owner_snapshot(
        &self,
        scope: &ProviderIncrementalScoped,
        owner_path: &str,
    ) -> Result<Option<ProviderIncrementalOwnerSnapshot>, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ReadProviderOwnerSnapshot {
                scope: scope.clone(),
                owner_path: owner_path.to_owned(),
            })
            .await?
        {
            WorkspaceDbIpcResult::ProviderOwnerSnapshot { snapshot } => Ok(snapshot),
            _ => Err("workspace owner IPC returned an unexpected owner snapshot result".to_owned()),
        }
    }

    pub async fn read_provider_owner_warm(
        &self,
        scope: &ProviderIncrementalScoped,
        owner: &ProviderOwnerBatchProbeRequest,
    ) -> Result<(ProviderOwnerProbe, Option<ProviderIncrementalOwnerSnapshot>), String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ReadProviderOwnerWarm {
                scope: scope.clone(),
                owner: owner.clone(),
            })
            .await?
        {
            WorkspaceDbIpcResult::ProviderOwnerWarm { probe, snapshot } => Ok((probe, snapshot)),
            _ => Err("workspace owner IPC returned an unexpected warm owner result".to_owned()),
        }
    }
}
