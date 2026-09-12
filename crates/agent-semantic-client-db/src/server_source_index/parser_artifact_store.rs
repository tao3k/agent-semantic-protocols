// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Content-addressed provider parser products, independent of generation proof.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use agent_semantic_provider_transport::projection_batch::ProviderProjectedOwner;
use serde::{Deserialize, Serialize};

const PARSER_ARTIFACT_SCHEMA_ID: &str = "agent.semantic-protocols.provider-parser-owner-artifact";
const PARSER_ARTIFACT_SCHEMA_VERSION: &str = "1";
const MAX_PARSER_ARTIFACT_BYTES: usize = 32 * 1024 * 1024;
static NEXT_PENDING_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ParserArtifactIdentity {
    pub provider_id: String,
    pub parser_identity_digest: String,
    pub query_pack_digest: String,
    pub auxiliary_input_digest: String,
    pub owner_path: String,
    pub owner_content_digest: String,
}

impl ParserArtifactIdentity {
    pub(super) fn key_digest(&self) -> Result<String, String> {
        let bytes = serde_json::to_vec(self)
            .map_err(|error| format!("encode parser artifact identity: {error}"))?;
        Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ParserArtifactBody {
    identity: ParserArtifactIdentity,
    owner: ProviderProjectedOwner,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ParserArtifactEnvelope {
    schema_id: String,
    schema_version: String,
    key_digest: String,
    artifact_digest: String,
    body: ParserArtifactBody,
}

#[derive(Clone, Debug)]
pub(super) struct ParserArtifactStore {
    root: PathBuf,
}

impl ParserArtifactStore {
    #[cfg(test)]
    pub(super) fn for_client_db(db_path: &Path) -> Result<Self, String> {
        let parent = db_path.parent().ok_or_else(|| {
            format!(
                "Client DB path has no parser artifact parent: {}",
                db_path.display()
            )
        })?;
        Ok(Self::for_artifact_root(parent))
    }

    pub(super) fn for_artifact_root(root: &Path) -> Self {
        Self {
            root: root.join("provider-parser-artifacts-v1"),
        }
    }

    pub(super) async fn read(
        &self,
        identity: &ParserArtifactIdentity,
    ) -> Result<Option<ProviderProjectedOwner>, String> {
        let key_digest = identity.key_digest()?;
        let path = self.path_for_digest(&key_digest)?;
        let bytes = match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(format!(
                    "read parser artifact `{}`: {error}",
                    path.display()
                ));
            }
        };
        if bytes.len() > MAX_PARSER_ARTIFACT_BYTES {
            return Err(format!(
                "parser artifact exceeds byte budget: path={} bytes={} limit={MAX_PARSER_ARTIFACT_BYTES}",
                path.display(),
                bytes.len(),
            ));
        }
        let envelope: ParserArtifactEnvelope = serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode parser artifact `{}`: {error}", path.display()))?;
        envelope.validate(identity, &key_digest)?;
        Ok(Some(envelope.body.owner))
    }

    pub(super) async fn publish(
        &self,
        identity: ParserArtifactIdentity,
        owner: ProviderProjectedOwner,
    ) -> Result<(), String> {
        let key_digest = identity.key_digest()?;
        let body = ParserArtifactBody { identity, owner };
        let artifact_digest = body_digest(&body)?;
        let envelope = ParserArtifactEnvelope {
            schema_id: PARSER_ARTIFACT_SCHEMA_ID.to_owned(),
            schema_version: PARSER_ARTIFACT_SCHEMA_VERSION.to_owned(),
            key_digest: key_digest.clone(),
            artifact_digest,
            body,
        };
        envelope.validate(&envelope.body.identity, &key_digest)?;
        let bytes = serde_json::to_vec(&envelope)
            .map_err(|error| format!("encode parser artifact: {error}"))?;
        if bytes.len() > MAX_PARSER_ARTIFACT_BYTES {
            return Err(format!(
                "parser artifact exceeds byte budget: bytes={} limit={MAX_PARSER_ARTIFACT_BYTES}",
                bytes.len()
            ));
        }
        tokio::fs::create_dir_all(&self.root)
            .await
            .map_err(|error| {
                format!(
                    "create parser artifact directory `{}`: {error}",
                    self.root.display()
                )
            })?;
        let final_path = self.path_for_digest(&key_digest)?;
        if let Ok(existing) = tokio::fs::read(&final_path).await
            && existing == bytes
        {
            return Ok(());
        }
        let pending_id = NEXT_PENDING_ID.fetch_add(1, Ordering::Relaxed);
        let pending_path = self.root.join(format!(
            ".{}.{}.{}.pending",
            std::process::id(),
            pending_id,
            final_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("parser-artifact")
        ));
        tokio::fs::write(&pending_path, &bytes)
            .await
            .map_err(|error| {
                format!(
                    "write parser artifact `{}`: {error}",
                    pending_path.display()
                )
            })?;
        tokio::fs::rename(&pending_path, &final_path)
            .await
            .map_err(|error| {
                format!(
                    "publish parser artifact `{}`: {error}",
                    final_path.display()
                )
            })?;
        Ok(())
    }

    fn path_for_digest(&self, digest: &str) -> Result<PathBuf, String> {
        let value = digest
            .strip_prefix("blake3-256:")
            .ok_or_else(|| "parser artifact key is not canonical".to_owned())?;
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err("parser artifact key is not canonical".to_owned());
        }
        Ok(self.root.join(format!("{value}.json")))
    }
}

impl ParserArtifactEnvelope {
    fn validate(
        &self,
        expected_identity: &ParserArtifactIdentity,
        expected_key_digest: &str,
    ) -> Result<(), String> {
        if self.schema_id != PARSER_ARTIFACT_SCHEMA_ID
            || self.schema_version != PARSER_ARTIFACT_SCHEMA_VERSION
            || &self.body.identity != expected_identity
            || self.key_digest != expected_key_digest
            || self.body.identity.key_digest()? != self.key_digest
            || body_digest(&self.body)? != self.artifact_digest
            || self.body.owner.owner_path != self.body.identity.owner_path
            || !same_digest(
                &self.body.owner.source_leaf_digest,
                &self.body.identity.owner_content_digest,
            )
        {
            return Err("parser artifact identity or content digest drift".to_owned());
        }
        Ok(())
    }
}

fn body_digest(body: &ParserArtifactBody) -> Result<String, String> {
    let bytes = serde_json::to_vec(body)
        .map_err(|error| format!("encode parser artifact body: {error}"))?;
    Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
}

fn same_digest(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix("blake3-256:")
            .is_some_and(|digest| digest == right)
        || right
            .strip_prefix("blake3-256:")
            .is_some_and(|digest| digest == left)
}

#[cfg(test)]
#[path = "../../tests/unit/parser_artifact_store.rs"]
mod tests;
