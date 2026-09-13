// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Canonical physical layout for Runtime artifact content and mutable publication state.
//!
//! Every path that can select, stage, activate, or fence Runtime executable content is
//! derived from this owner.  Callers must not reconstruct sibling `runtime/*` paths.

use std::fs;
use std::path::{Path, PathBuf};

/// Return whether a canonical Runtime member path belongs to an immutable,
/// bundle-digest-addressed generation under `artifact_root`.
pub fn runtime_artifact_member_is_digest_addressed(
    identity: &Path,
    artifact_root: &Path,
) -> Result<bool, String> {
    let artifact_root = match fs::canonicalize(artifact_root) {
        Ok(artifact_root) => artifact_root,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "failed to resolve protocol artifact root {}: {error}",
                artifact_root.display()
            ));
        }
    };
    let Ok(relative) = identity.strip_prefix(&artifact_root) else {
        return Ok(false);
    };
    let mut components = relative.components();
    let Some(store) = components
        .next()
        .and_then(|value| value.as_os_str().to_str())
    else {
        return Ok(false);
    };
    let Some(digest) = components
        .next()
        .and_then(|value| value.as_os_str().to_str())
    else {
        return Ok(false);
    };
    let Some(member) = components.next().map(|value| value.as_os_str()) else {
        return Ok(false);
    };
    Ok(store == "generations"
        && valid_blake3_digest(digest)
        && !member.is_empty()
        && components.next().is_none())
}

/// Project the owning Runtime bundle digest from a canonical generation member.
pub fn runtime_artifact_bundle_digest_from_member_path(canonical: &Path) -> Option<String> {
    let generation = canonical.parent()?;
    let store = generation.parent()?;
    if store.file_name()?.to_str()? != "generations" {
        return None;
    }
    let digest = generation.file_name()?.to_str()?;
    valid_blake3_digest(digest).then(|| digest.to_owned())
}

fn valid_blake3_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

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
