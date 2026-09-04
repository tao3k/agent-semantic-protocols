//! Canonical physical State Home layout owned by Artifacts.

use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::WorkspaceIdentity;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateHomeLayout {
    pub root: PathBuf,
    pub catalog: PathBuf,
    pub workspaces: PathBuf,
    pub blobs: PathBuf,
    pub runtime: PathBuf,
    pub receipts: PathBuf,
    pub trash: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatePaths {
    pub root: PathBuf,
    pub facts: PathBuf,
    pub artifacts: PathBuf,
    pub observations: PathBuf,
}

impl StateHomeLayout {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        Self {
            catalog: root.join("catalog").join("state.turso"),
            workspaces: root.join("workspaces"),
            blobs: root.join("blobs").join("blake3-256"),
            runtime: root.join("runtime"),
            receipts: root.join("receipts"),
            trash: root.join("trash"),
            root,
        }
    }

    pub fn workspace(&self, identity: &WorkspaceIdentity) -> Result<WorkspaceStatePaths, String> {
        let digest = identity
            .digest
            .as_str()
            .strip_prefix("blake3-256:")
            .ok_or_else(|| "workspace digest must use canonical blake3-256".to_string())?;
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("workspace digest must contain 64 hexadecimal digits".to_string());
        }
        let root = self.workspaces.join(digest);
        Ok(WorkspaceStatePaths {
            facts: root.join("facts.turso"),
            artifacts: root.join("artifacts"),
            observations: root.join("observations"),
            root,
        })
    }
}
