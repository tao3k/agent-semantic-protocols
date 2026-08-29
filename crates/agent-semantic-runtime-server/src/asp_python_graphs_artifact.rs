//! Verified, immutable artifact contract for the resident ASP Python Graphs entrypoint.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const ARTIFACT_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-python-graphs-runtime-artifact";
pub const ARTIFACT_SCHEMA_VERSION: &str = "1";
pub const PROTOCOL_NAMESPACE: &str = "asp.python.graphs";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspPythonGraphsArtifactDescriptor {
    pub schema_id: String,
    pub schema_version: String,
    pub artifact_kind: String,
    pub protocol_id: String,
    pub protocol_version: String,
    pub execution_artifact_locator: PathBuf,
    pub command_arguments: Vec<String>,
    pub content_digest: String,
    pub execution_command_digest: String,
    pub artifact_metadata_digest: String,
}

#[derive(Clone, Debug)]
pub struct VerifiedAspPythonGraphsArtifact {
    descriptor: AspPythonGraphsArtifactDescriptor,
}

impl AspPythonGraphsArtifactDescriptor {
    pub async fn load(path: impl AsRef<Path>) -> Result<VerifiedAspPythonGraphsArtifact, String> {
        let descriptor_path = path.as_ref();
        let bytes = tokio::fs::read(descriptor_path).await.map_err(|error| {
            format!("asp-python-graphs artifact descriptor read failed: {error}")
        })?;
        let descriptor: Self = serde_json::from_slice(&bytes).map_err(|error| {
            format!("asp-python-graphs artifact descriptor decode failed: {error}")
        })?;
        descriptor.verify().await
    }

    pub async fn verify(self) -> Result<VerifiedAspPythonGraphsArtifact, String> {
        if self.schema_id != ARTIFACT_SCHEMA_ID || self.schema_version != ARTIFACT_SCHEMA_VERSION {
            return Err("state=unavailable reasonKind=asp-python-graphs-artifact-schema-mismatch".to_owned());
        }
        if self.artifact_kind != "standalone-executable"
            || self.protocol_id != PROTOCOL_NAMESPACE
            || self.protocol_version != ARTIFACT_SCHEMA_VERSION
        {
            return Err("state=unavailable reasonKind=asp-python-graphs-artifact-contract-mismatch".to_owned());
        }
        if !self.execution_artifact_locator.is_absolute() {
            return Err("state=unavailable reasonKind=asp-python-graphs-artifact-locator-not-absolute".to_owned());
        }
        if self.command_arguments.is_empty() {
            return Err("state=unavailable reasonKind=asp-python-graphs-artifact-command-missing".to_owned());
        }
        for (field, digest) in [
            ("contentDigest", &self.content_digest),
            ("executionCommandDigest", &self.execution_command_digest),
            ("artifactMetadataDigest", &self.artifact_metadata_digest),
        ] {
            if !valid_digest(digest) {
                return Err(format!("state=unavailable reasonKind=asp-python-graphs-{field}-invalid"));
            }
        }
        let metadata = tokio::fs::metadata(&self.execution_artifact_locator)
            .await
            .map_err(|error| format!("state=unavailable reasonKind=asp-python-graphs-artifact-missing error={error}"))?;
        if !metadata.is_file() {
            return Err("state=unavailable reasonKind=asp-python-graphs-artifact-not-file".to_owned());
        }
        let bytes = tokio::fs::read(&self.execution_artifact_locator)
            .await
            .map_err(|error| format!("state=unavailable reasonKind=asp-python-graphs-artifact-read-failed error={error}"))?;
        let observed = format!("blake3-256:{}", blake3::hash(&bytes).to_hex());
        if observed != self.content_digest {
            return Err(format!(
                "state=unavailable reasonKind=asp-python-graphs-artifact-digest-mismatch expected={} observed={observed}",
                self.content_digest
            ));
        }
        let command_material = serde_json::json!({
            "executable": &self.execution_artifact_locator,
            "arguments": &self.command_arguments,
        });
        let observed_command = digest_json(&command_material);
        if observed_command != self.execution_command_digest {
            return Err(format!(
                "state=unavailable reasonKind=asp-python-graphs-command-digest-mismatch expected={} observed={observed_command}",
                self.execution_command_digest
            ));
        }
        let metadata_material = serde_json::json!({
            "schemaId": self.schema_id,
            "schemaVersion": self.schema_version,
            "artifactKind": self.artifact_kind,
            "protocolId": self.protocol_id,
            "protocolVersion": self.protocol_version,
            "executionArtifactLocator": &self.execution_artifact_locator,
            "commandArguments": &self.command_arguments,
            "contentDigest": &self.content_digest,
            "executionCommandDigest": &self.execution_command_digest,
        });
        let observed_metadata = digest_json(&metadata_material);
        if observed_metadata != self.artifact_metadata_digest {
            return Err(format!(
                "state=unavailable reasonKind=asp-python-graphs-metadata-digest-mismatch expected={} observed={observed_metadata}",
                self.artifact_metadata_digest
            ));
        }
        Ok(VerifiedAspPythonGraphsArtifact { descriptor: self })
    }
}

impl VerifiedAspPythonGraphsArtifact {
    pub fn descriptor(&self) -> &AspPythonGraphsArtifactDescriptor { &self.descriptor }

    pub fn command_for_socket(&self, socket_path: &Path) -> Result<tokio::process::Command, String> {
        if !socket_path.is_absolute() {
            return Err("asp-python-graphs socket locator must be absolute".to_owned());
        }
        let mut command = tokio::process::Command::new(&self.descriptor.execution_artifact_locator);
        let mut placeholder_count = 0usize;
        for argument in &self.descriptor.command_arguments {
            placeholder_count += argument.matches("${socketPath}").count();
            if argument.contains("${") && !argument.contains("${socketPath}") {
                return Err("asp-python-graphs artifact command contains unresolved placeholder".to_owned());
            }
            command.arg(argument.replace("${socketPath}", &socket_path.to_string_lossy()));
        }
        if placeholder_count != 1 {
            return Err("asp-python-graphs artifact command requires exactly one socketPath placeholder".to_owned());
        }
        command.env_clear();
        command.kill_on_drop(true);
        Ok(command)
    }
}

fn valid_digest(value: &str) -> bool {
    value.starts_with("blake3-256:") && value.len() == 75
        && value[11..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn digest_json(value: &serde_json::Value) -> String {
    let bytes = serde_json::to_vec(value).expect("canonical artifact digest input is serializable");
    format!("blake3-256:{}", blake3::hash(&bytes).to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(bytes: &[u8]) -> String {
        format!("blake3-256:{}", blake3::hash(bytes).to_hex())
    }

    fn descriptor(path: PathBuf, content: &[u8]) -> AspPythonGraphsArtifactDescriptor {
        AspPythonGraphsArtifactDescriptor {
            schema_id: ARTIFACT_SCHEMA_ID.to_owned(),
            schema_version: ARTIFACT_SCHEMA_VERSION.to_owned(),
            artifact_kind: "standalone-executable".to_owned(),
            protocol_id: PROTOCOL_NAMESPACE.to_owned(),
            protocol_version: ARTIFACT_SCHEMA_VERSION.to_owned(),
            execution_artifact_locator: path.clone(),
            command_arguments: vec!["--socket=${socketPath}".to_owned()],
            content_digest: digest(content),
            execution_command_digest: digest_json(&serde_json::json!({
                "executable": &path,
                "arguments": ["--socket=${socketPath}"],
            })),
            artifact_metadata_digest: digest_json(&serde_json::json!({
                "schemaId": ARTIFACT_SCHEMA_ID,
                "schemaVersion": ARTIFACT_SCHEMA_VERSION,
                "artifactKind": "standalone-executable",
                "protocolId": PROTOCOL_NAMESPACE,
                "protocolVersion": ARTIFACT_SCHEMA_VERSION,
                "executionArtifactLocator": &path,
                "commandArguments": ["--socket=${socketPath}"],
                "contentDigest": digest(content),
                "executionCommandDigest": digest_json(&serde_json::json!({
                    "executable": path,
                    "arguments": ["--socket=${socketPath}"],
                })),
            })),
        }
    }

    #[tokio::test]
    async fn descriptor_verifies_absolute_entrypoint_and_content_digest() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("asp-python-graphs");
        let content = b"immutable-entrypoint";
        tokio::fs::write(&path, content).await.expect("entrypoint");
        let verified = descriptor(path, content).verify().await.expect("verified descriptor");
        let command = verified
            .command_for_socket(&directory.path().join("graphs.sock"))
            .expect("command");
        assert_eq!(command.as_std().get_program(), directory.path().join("asp-python-graphs"));
    }

    #[tokio::test]
    async fn descriptor_digest_mismatch_is_typed_unavailable() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("asp-python-graphs");
        tokio::fs::write(&path, b"actual").await.expect("entrypoint");
        let error = descriptor(path, b"expected").verify().await.expect_err("digest mismatch");
        assert!(error.contains("artifact-digest-mismatch"));
    }
}
