// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Canonical physical layout for Runtime artifact content and mutable publication state.
//!
//! Every path that can select, stage, activate, or fence Runtime executable content is
//! derived from this owner.  Callers must not reconstruct sibling `runtime/*` paths.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactStateLayout {
    root: PathBuf,
}

impl RuntimeArtifactStateLayout {
    pub fn new(state_home: impl AsRef<Path>) -> Self {
        let state_home = state_home.as_ref().to_path_buf();
        let root = state_home.join("runtime/artifacts");
        Self { root }
    }

    pub(crate) fn from_artifact_root(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Immutable Runtime execution generations keyed by the bundle digest.
    ///
    /// Member content digests are leaves of each generation manifest. They do
    /// not own a second persistent CAS or serving selector.
    pub fn generation_store(&self) -> PathBuf {
        self.root.join("generations")
    }

    /// Transaction-local inputs that are not serving authorities.
    pub fn staging(&self) -> PathBuf {
        self.root.join("staging")
    }

    pub fn provider_staging(&self) -> PathBuf {
        self.staging().join("providers")
    }

    pub fn active_slot(&self) -> PathBuf {
        self.root.join("active")
    }

    pub fn healthy_slot(&self) -> PathBuf {
        self.root.join("healthy")
    }

    pub fn activation(&self) -> PathBuf {
        self.root.join("activation")
    }

    pub fn pending_activation(&self) -> PathBuf {
        self.activation().join("pending.json")
    }

    pub fn applied_activation(&self) -> PathBuf {
        self.activation().join("applied.json")
    }

    pub fn leases(&self) -> PathBuf {
        self.root.join("leases")
    }

    pub fn publication_lease(&self) -> PathBuf {
        self.leases().join("artifact-publication.v1.json")
    }
}
