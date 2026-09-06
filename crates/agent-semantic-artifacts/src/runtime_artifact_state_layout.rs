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

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn content_store(&self) -> PathBuf {
        self.root.join("blake3-256")
    }

    /// Content-addressed provider artifacts share the Runtime artifact
    /// authority. Provider identity is a namespace inside the artifact store,
    /// never a sibling Runtime root with independent active/healthy pointers.
    pub fn provider_content_store(&self) -> PathBuf {
        self.root.join("providers")
    }

    pub fn bundle_store(&self) -> PathBuf {
        self.root.join("bundles")
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
