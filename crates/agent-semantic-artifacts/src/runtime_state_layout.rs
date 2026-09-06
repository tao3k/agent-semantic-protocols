// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Canonical typed layout for generated Runtime State Home objects.
//!
//! Domain packages own transition semantics. This module is the sole owner of
//! physical Runtime path derivation.

use std::path::{Path, PathBuf};

use crate::RuntimeArtifactStateLayout;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStateLayout {
    state_home: PathBuf,
    root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeServingStateLayout {
    root: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeLifecycleReceiptName {
    RunIntent,
    OperatorStop,
    OwnerSpawn,
    DaemonExit,
    DaemonDrain,
}

impl RuntimeStateLayout {
    pub(crate) fn new(state_home: impl AsRef<Path>) -> Self {
        let state_home = state_home.as_ref().to_path_buf();
        let root = state_home.join("runtime");
        Self { state_home, root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn artifacts(&self) -> RuntimeArtifactStateLayout {
        RuntimeArtifactStateLayout::new(&self.state_home)
    }

    /// Stable executable entrypoints derived from the active artifact slot.
    pub fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }

    /// Managed source checkouts used to build source-index generations.
    pub fn sources(&self) -> PathBuf {
        self.root.join("sources")
    }

    pub fn serving(&self) -> RuntimeServingStateLayout {
        RuntimeServingStateLayout {
            root: self.root.join("serving"),
        }
    }
}

impl RuntimeServingStateLayout {
    /// Constructs an explicitly injected serving root for tests and Host-owned
    /// publication boundaries. Production code should start from `StateHomeLayout`.
    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn from_injected_publication_root(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into().join("lifecycle"),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn endpoint_receipt(&self) -> PathBuf {
        self.root.join("endpoint.v1.json")
    }

    pub fn injected_endpoint_receipt(&self) -> PathBuf {
        self.root.join("endpoint.json")
    }

    pub fn readiness(&self) -> PathBuf {
        self.root.join("readiness")
    }

    pub fn workspaces(&self) -> PathBuf {
        self.root.join("workspaces")
    }

    pub fn mailboxes(&self) -> PathBuf {
        self.root.join("mailboxes")
    }

    pub fn hook_host_native_handoff_mailbox(&self) -> PathBuf {
        self.mailboxes().join("hook-host-native-handoff")
    }

    pub fn hook_break_glass_mailbox(&self) -> PathBuf {
        self.mailboxes().join("hook-break-glass")
    }

    pub fn owner_election_lock(&self) -> PathBuf {
        self.root.join("runtime-server.owner.lock")
    }

    pub fn supervisor_transaction_lock(&self) -> PathBuf {
        self.root.join("runtime-server.supervisor.lock")
    }

    pub fn owner_spawn_receipt(&self) -> PathBuf {
        self.root.join("owner-spawn.v1.json")
    }

    pub fn owner_stderr_log(&self) -> PathBuf {
        self.root.join("owner-stderr.log")
    }

    pub fn provider_register_receipt(&self) -> PathBuf {
        self.root.join("provider-register.v1.json")
    }

    pub fn workspace_admission_catalog(&self) -> PathBuf {
        self.root.join("workspace-admissions.v1.json")
    }

    pub fn diagnostic_receipt(&self) -> PathBuf {
        self.root.join("runtime-server-diagnostic.v1.json")
    }

    pub fn identity_monitor_receipt(&self) -> PathBuf {
        self.root.join("identity-monitor.v1.json")
    }

    pub fn python_graphs_socket(&self) -> PathBuf {
        self.root.join("asp-python-graphs.sock")
    }

    pub fn opentelemetry_socket(&self) -> PathBuf {
        self.root.join("opentelemetry.sock")
    }

    pub fn opentelemetry_query_socket(&self) -> PathBuf {
        self.root.join("opentelemetry-query.sock")
    }

    pub fn lifecycle_receipt(&self, name: RuntimeLifecycleReceiptName) -> PathBuf {
        let file_name = match name {
            RuntimeLifecycleReceiptName::RunIntent => "run-intent.v1",
            RuntimeLifecycleReceiptName::OperatorStop => "operator-stop.v1.json",
            RuntimeLifecycleReceiptName::OwnerSpawn => "owner-spawn.v1.json",
            RuntimeLifecycleReceiptName::DaemonExit => "daemon-exit.v1.json",
            RuntimeLifecycleReceiptName::DaemonDrain => "daemon-drain.v1.json",
        };
        self.root.join(file_name)
    }

    pub fn status_memory(
        &self,
        owner_epoch: u64,
        binding_token: &str,
        runtime_binary_content_digest: impl std::fmt::Display,
    ) -> PathBuf {
        let digest = blake3::hash(
            format!("{owner_epoch}\0{binding_token}\0{runtime_binary_content_digest}").as_bytes(),
        )
        .to_hex();
        self.root.join(format!("status-{}.memory", &digest[..16]))
    }
}
