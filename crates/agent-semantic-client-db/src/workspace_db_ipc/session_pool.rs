// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Process-resident IPC lanes keyed by Runtime Server workspace binding.

use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use super::protocol::{WorkspaceDbIpcSessionState, WorkspaceDbSessionBinding};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ResidentSessionKey {
    workspace_identity: String,
    project_root: Option<PathBuf>,
    data_endpoint: crate::runtime_server_control::RuntimeServerLoopbackEndpoint,
    transport_contract_digest: String,
    owner_epoch: u64,
    binding_token: String,
}

impl From<&WorkspaceDbSessionBinding> for ResidentSessionKey {
    fn from(binding: &WorkspaceDbSessionBinding) -> Self {
        Self {
            workspace_identity: binding.workspace_identity.clone(),
            project_root: binding.project_root.clone(),
            data_endpoint: binding.data_endpoint.clone(),
            transport_contract_digest: binding.transport_contract_digest.clone(),
            owner_epoch: binding.owner_epoch,
            binding_token: binding.binding_token.clone(),
        }
    }
}

static RESIDENT_SESSIONS: LazyLock<
    dashmap::DashMap<ResidentSessionKey, Arc<WorkspaceDbIpcSessionState>>,
> = LazyLock::new(dashmap::DashMap::new);

pub(super) fn resident_state(
    binding: &WorkspaceDbSessionBinding,
    create: impl FnOnce() -> Arc<WorkspaceDbIpcSessionState>,
) -> Arc<WorkspaceDbIpcSessionState> {
    let key = ResidentSessionKey::from(binding);
    if let Some(existing) = RESIDENT_SESSIONS.get(&key) {
        return Arc::clone(existing.value());
    }
    if RESIDENT_SESSIONS.len() >= 64 {
        RESIDENT_SESSIONS.retain(|existing, _| {
            existing.workspace_identity != key.workspace_identity
                || existing.project_root != key.project_root
                || existing.data_endpoint != key.data_endpoint
                || existing == &key
        });
    }
    Arc::clone(RESIDENT_SESSIONS.entry(key).or_insert_with(create).value())
}
