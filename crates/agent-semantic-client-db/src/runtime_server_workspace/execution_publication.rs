// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Durable single-authority storage for the source/Runtime execution product.

use std::path::{Path, PathBuf};

use agent_semantic_content_identity::runtime_workspace_execution_pointer::RuntimeWorkspaceExecutionPointer;
use agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication;
use tokio::io::AsyncWriteExt;

use super::atomic_snapshot_pointer::{AtomicSnapshotPointerReader, AtomicSnapshotPointerWriter};

const PUBLICATION_DIRECTORY: &str = "execution-publications/v1";
const POINTER_FILE: &str = "active-execution.pointer";
const POINTER_CONTEXT: &str = "runtime workspace execution";

/// Canonical durable store for immutable execution sidecars and their one active pointer.
#[derive(Debug, Clone)]
pub struct RuntimeWorkspaceExecutionPublicationStore {
    publications: PathBuf,
    pointer: AtomicSnapshotPointerWriter,
}

impl RuntimeWorkspaceExecutionPublicationStore {
    /// Opens the canonical writer rooted at one workspace generation directory.
    pub async fn open(root: &Path) -> Result<Self, String> {
        let publications = root.join(PUBLICATION_DIRECTORY);
        tokio::fs::create_dir_all(&publications)
            .await
            .map_err(|error| {
                format!(
                    "create runtime workspace execution publication directory `{}`: {error}",
                    publications.display()
                )
            })?;
        let pointer =
            AtomicSnapshotPointerWriter::open(root.join(POINTER_FILE), POINTER_CONTEXT).await?;
        Ok(Self {
            publications,
            pointer,
        })
    }

    /// Writes and fsyncs the immutable sidecar before replacing the active pointer.
    pub async fn publish(
        &self,
        publication: &RuntimeWorkspaceExecutionPublication,
    ) -> Result<RuntimeWorkspaceExecutionPointer, String> {
        publication.validate().map_err(|error| {
            format!("validate runtime workspace execution publication: {error:?}")
        })?;
        let pointer = RuntimeWorkspaceExecutionPointer::from_publication(publication)
            .map_err(|error| format!("build runtime workspace execution pointer: {error:?}"))?;
        self.publish_immutable_sidecar(publication).await?;
        self.pointer.publish(&pointer).await?;
        Ok(pointer)
    }

    /// Reads the active pointer and admits only the exact immutable sidecar it names.
    pub async fn read_active(root: &Path) -> Result<RuntimeWorkspaceExecutionPublication, String> {
        Self::read_active_optional(root).await?.ok_or_else(|| {
            "runtime workspace execution pointer has no complete generation".to_owned()
        })
    }

    /// Reads the current execution product without treating an uninitialized
    /// pointer as a predecessor. Invalid or incomplete published products still
    /// fail closed.
    pub async fn read_active_optional(
        root: &Path,
    ) -> Result<Option<RuntimeWorkspaceExecutionPublication>, String> {
        let pointer_path = root.join(POINTER_FILE);
        let Some(reader) =
            AtomicSnapshotPointerReader::<RuntimeWorkspaceExecutionPointer>::open_optional(
                &pointer_path,
                POINTER_CONTEXT,
            )
            .await?
        else {
            return Ok(None);
        };
        let Some(pointer) = reader.read_optional()? else {
            return Ok(None);
        };
        pointer
            .validate()
            .map_err(|error| format!("validate runtime workspace execution pointer: {error:?}"))?;
        let sidecar_path = sidecar_path_for_digest(
            &root.join(PUBLICATION_DIRECTORY),
            pointer.execution_publication_digest.as_str(),
        )?;
        let bytes = tokio::fs::read(&sidecar_path).await.map_err(|error| {
            format!(
                "read runtime workspace execution publication sidecar `{}`: {error}",
                sidecar_path.display()
            )
        })?;
        let publication: RuntimeWorkspaceExecutionPublication = serde_json::from_slice(&bytes)
            .map_err(|error| {
                format!(
                    "decode runtime workspace execution publication sidecar `{}`: {error}",
                    sidecar_path.display()
                )
            })?;
        pointer
            .admits(&publication)
            .map_err(|error| format!("admit runtime workspace execution sidecar: {error:?}"))?;
        Ok(Some(publication))
    }

    #[doc(hidden)]
    pub fn sidecar_path(&self, publication: &RuntimeWorkspaceExecutionPublication) -> PathBuf {
        sidecar_path_for_digest(&self.publications, publication.publication_digest.as_str())
            .expect("validated publication digest has a canonical sidecar path")
    }

    async fn publish_immutable_sidecar(
        &self,
        publication: &RuntimeWorkspaceExecutionPublication,
    ) -> Result<(), String> {
        let final_path = self.sidecar_path(publication);
        let bytes = serde_json::to_vec(publication)
            .map_err(|error| format!("encode runtime workspace execution publication: {error}"))?;
        match tokio::fs::read(&final_path).await {
            Ok(existing) if existing == bytes => return Ok(()),
            Ok(_) => {
                return Err(format!(
                    "immutable runtime workspace execution publication collision: `{}`",
                    final_path.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "inspect runtime workspace execution publication sidecar `{}`: {error}",
                    final_path.display()
                ));
            }
        }
        let temporary_path = final_path.with_extension("pending");
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary_path)
            .await
            .map_err(|error| {
                format!(
                    "create runtime workspace execution publication sidecar `{}`: {error}",
                    temporary_path.display()
                )
            })?;
        file.write_all(&bytes).await.map_err(|error| {
            format!(
                "write runtime workspace execution publication sidecar `{}`: {error}",
                temporary_path.display()
            )
        })?;
        file.sync_all().await.map_err(|error| {
            format!(
                "sync runtime workspace execution publication sidecar `{}`: {error}",
                temporary_path.display()
            )
        })?;
        drop(file);
        tokio::fs::rename(&temporary_path, &final_path)
            .await
            .map_err(|error| {
                format!(
                    "publish runtime workspace execution publication sidecar `{}`: {error}",
                    final_path.display()
                )
            })?;
        tokio::fs::File::open(&self.publications)
            .await
            .map_err(|error| {
                format!(
                    "open runtime workspace execution publication directory `{}`: {error}",
                    self.publications.display()
                )
            })?
            .sync_all()
            .await
            .map_err(|error| {
                format!(
                    "sync runtime workspace execution publication directory `{}`: {error}",
                    self.publications.display()
                )
            })?;
        Ok(())
    }
}

fn sidecar_path_for_digest(directory: &Path, digest: &str) -> Result<PathBuf, String> {
    let Some(hex) = digest.strip_prefix("blake3-256:") else {
        return Err("runtime workspace execution publication digest is not canonical".to_owned());
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("runtime workspace execution publication digest is not canonical".to_owned());
    }
    Ok(directory.join(format!("{hex}.json")))
}
