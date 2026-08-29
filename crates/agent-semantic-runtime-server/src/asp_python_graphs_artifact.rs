//! Verified, immutable artifact contract for the resident ASP Python Graphs entrypoint.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const ARTIFACT_SCHEMA_ID: &str = "agent.semantic-protocols.asp-python-graphs-runtime-artifact";
pub const ARTIFACT_SCHEMA_VERSION: &str = "2";
pub const PROTOCOL_NAMESPACE: &str = "asp.python.graphs";
pub const PROTOCOL_VERSION: &str = "1";
pub const ARTIFACT_DESCRIPTOR_FILE: &str = "asp-python-graphs-artifact.v2.json";

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
    pub publication_generation: u64,
    pub publication_nonce: String,
}

#[derive(Clone, Debug)]
pub struct VerifiedAspPythonGraphsArtifact {
    descriptor: AspPythonGraphsArtifactDescriptor,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AspPythonGraphsArtifactPublicationReceipt {
    pub path: PathBuf,
    pub state: &'static str,
    pub publication_generation: u64,
    pub publication_nonce: String,
    pub content_digest: String,
    pub execution_command_digest: String,
    pub artifact_metadata_digest: String,
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
            return Err(
                "state=unavailable reasonKind=asp-python-graphs-artifact-schema-mismatch"
                    .to_owned(),
            );
        }
        if self.artifact_kind != "standalone-executable"
            || self.protocol_id != PROTOCOL_NAMESPACE
            || self.protocol_version != PROTOCOL_VERSION
        {
            return Err(
                "state=unavailable reasonKind=asp-python-graphs-artifact-contract-mismatch"
                    .to_owned(),
            );
        }
        if !self.execution_artifact_locator.is_absolute() {
            return Err(
                "state=unavailable reasonKind=asp-python-graphs-artifact-locator-not-absolute"
                    .to_owned(),
            );
        }
        if self.command_arguments.is_empty() {
            return Err(
                "state=unavailable reasonKind=asp-python-graphs-artifact-command-missing"
                    .to_owned(),
            );
        }
        for (field, digest) in [
            ("contentDigest", &self.content_digest),
            ("executionCommandDigest", &self.execution_command_digest),
            ("artifactMetadataDigest", &self.artifact_metadata_digest),
        ] {
            if !valid_digest(digest) {
                return Err(format!(
                    "state=unavailable reasonKind=asp-python-graphs-{field}-invalid"
                ));
            }
        }
        if self.publication_generation == 0 || self.publication_nonce.is_empty() {
            return Err(
                "state=unavailable reasonKind=asp-python-graphs-publication-identity-invalid"
                    .to_owned(),
            );
        }
        let metadata = tokio::fs::metadata(&self.execution_artifact_locator)
            .await
            .map_err(|error| {
                format!(
                    "state=unavailable reasonKind=asp-python-graphs-artifact-missing error={error}"
                )
            })?;
        if !metadata.is_file() {
            return Err(
                "state=unavailable reasonKind=asp-python-graphs-artifact-not-file".to_owned(),
            );
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
            "publicationGeneration": self.publication_generation,
            "publicationNonce": &self.publication_nonce,
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

pub async fn publish_asp_python_graphs_artifact(
    state_home: &Path,
    executable: &Path,
    command_arguments: Vec<String>,
    expected_generation: Option<u64>,
) -> Result<AspPythonGraphsArtifactPublicationReceipt, String> {
    let state_home = state_home.to_path_buf();
    let executable = executable.to_path_buf();
    let receipt = tokio::task::spawn_blocking(move || {
        publish_asp_python_graphs_artifact_blocking(
            &state_home,
            &executable,
            command_arguments,
            expected_generation,
        )
    })
    .await
    .map_err(|error| format!("asp-python-graphs artifact publication task failed: {error}"))??;
    AspPythonGraphsArtifactDescriptor::load(&receipt.path).await?;
    Ok(receipt)
}

fn publish_asp_python_graphs_artifact_blocking(
    state_home: &Path,
    executable: &Path,
    command_arguments: Vec<String>,
    expected_generation: Option<u64>,
) -> Result<AspPythonGraphsArtifactPublicationReceipt, String> {
    use std::io::Write;

    let server_dir = state_home.join("runtime/server");
    std::fs::create_dir_all(&server_dir).map_err(|error| {
        format!("create asp-python-graphs artifact publication directory: {error}")
    })?;
    let artifact_root = state_home.join("runtime/artifacts");
    std::fs::create_dir_all(&artifact_root)
        .map_err(|error| format!("create Runtime artifact root: {error}"))?;
    let _mutation_guard = agent_semantic_artifacts::runtime_artifact_retention::RuntimeArtifactMutationGuard::try_acquire(
        &artifact_root,
    )?;
    let descriptor_path = server_dir.join(ARTIFACT_DESCRIPTOR_FILE);
    let current_generation = match std::fs::read(&descriptor_path) {
        Ok(bytes) => serde_json::from_slice::<AspPythonGraphsArtifactDescriptor>(&bytes)
            .map_err(|error| {
                format!(
                    "state=failed reasonKind=asp-python-graphs-existing-descriptor-invalid path={} error={error}",
                    descriptor_path.display()
                )
            })?
            .publication_generation,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(format!("read current asp-python-graphs descriptor: {error}")),
    };
    if expected_generation.is_some_and(|expected| expected != current_generation) {
        return Err(format!(
            "state=failed reasonKind=asp-python-graphs-stale-writer expectedGeneration={} currentGeneration={current_generation}",
            expected_generation.unwrap_or_default()
        ));
    }
    let publication_generation = current_generation
        .checked_add(1)
        .ok_or_else(|| "asp-python-graphs publication generation overflow".to_owned())?;
    let executable = std::fs::canonicalize(executable)
        .map_err(|error| format!("canonicalize asp-python-graphs executable: {error}"))?;
    if !executable.is_file() {
        return Err("asp-python-graphs execution artifact must be a file".to_owned());
    }
    let placeholder_count = command_arguments
        .iter()
        .map(|argument| argument.matches("${socketPath}").count())
        .sum::<usize>();
    if command_arguments.is_empty() || placeholder_count != 1 {
        return Err(
            "asp-python-graphs command requires exactly one socketPath placeholder".to_owned(),
        );
    }
    let content = std::fs::read(&executable)
        .map_err(|error| format!("read asp-python-graphs executable: {error}"))?;
    let content_digest = format!("blake3-256:{}", blake3::hash(&content).to_hex());
    let execution_command_digest = digest_json(&serde_json::json!({
        "executable": &executable,
        "arguments": &command_arguments,
    }));
    let unix_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("resolve artifact publication time: {error}"))?
        .as_nanos();
    let publication_nonce = format!(
        "graphs-{}-{unix_nanos}-{publication_generation}",
        std::process::id()
    );
    let metadata_material = serde_json::json!({
        "schemaId": ARTIFACT_SCHEMA_ID,
        "schemaVersion": ARTIFACT_SCHEMA_VERSION,
        "artifactKind": "standalone-executable",
        "protocolId": PROTOCOL_NAMESPACE,
        "protocolVersion": PROTOCOL_VERSION,
        "executionArtifactLocator": &executable,
        "commandArguments": &command_arguments,
        "contentDigest": &content_digest,
        "executionCommandDigest": &execution_command_digest,
        "publicationGeneration": publication_generation,
        "publicationNonce": &publication_nonce,
    });
    let artifact_metadata_digest = digest_json(&metadata_material);
    let descriptor = AspPythonGraphsArtifactDescriptor {
        schema_id: ARTIFACT_SCHEMA_ID.to_owned(),
        schema_version: ARTIFACT_SCHEMA_VERSION.to_owned(),
        artifact_kind: "standalone-executable".to_owned(),
        protocol_id: PROTOCOL_NAMESPACE.to_owned(),
        protocol_version: PROTOCOL_VERSION.to_owned(),
        execution_artifact_locator: executable,
        command_arguments,
        content_digest: content_digest.clone(),
        execution_command_digest: execution_command_digest.clone(),
        artifact_metadata_digest: artifact_metadata_digest.clone(),
        publication_generation,
        publication_nonce: publication_nonce.clone(),
    };
    let mut bytes = serde_json::to_vec_pretty(&descriptor)
        .map_err(|error| format!("encode asp-python-graphs descriptor: {error}"))?;
    bytes.push(b'\n');
    let temporary_path = server_dir.join(format!(
        ".{ARTIFACT_DESCRIPTOR_FILE}.{publication_nonce}.tmp"
    ));
    let publication = (|| -> Result<(), String> {
        let mut temporary = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|error| format!("create artifact descriptor temporary file: {error}"))?;
        temporary
            .write_all(&bytes)
            .map_err(|error| format!("write artifact descriptor temporary file: {error}"))?;
        temporary
            .sync_all()
            .map_err(|error| format!("sync artifact descriptor temporary file: {error}"))?;
        std::fs::rename(&temporary_path, &descriptor_path)
            .map_err(|error| format!("atomically publish artifact descriptor: {error}"))?;
        std::fs::File::open(&server_dir)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("sync artifact descriptor directory: {error}"))?;
        Ok(())
    })();
    if publication.is_err() {
        let _ = std::fs::remove_file(&temporary_path);
    }
    publication?;
    Ok(AspPythonGraphsArtifactPublicationReceipt {
        path: descriptor_path,
        state: "published",
        publication_generation,
        publication_nonce,
        content_digest,
        execution_command_digest,
        artifact_metadata_digest,
    })
}

impl VerifiedAspPythonGraphsArtifact {
    pub fn descriptor(&self) -> &AspPythonGraphsArtifactDescriptor {
        &self.descriptor
    }

    pub fn command_for_socket(
        &self,
        socket_path: &Path,
    ) -> Result<tokio::process::Command, String> {
        if !socket_path.is_absolute() {
            return Err("asp-python-graphs socket locator must be absolute".to_owned());
        }
        let bytes = std::fs::read(&self.descriptor.execution_artifact_locator)
            .map_err(|error| format!("state=unavailable reasonKind=asp-python-graphs-artifact-read-failed error={error}"))?;
        let observed = format!("blake3-256:{}", blake3::hash(&bytes).to_hex());
        if observed != self.descriptor.content_digest {
            return Err(format!(
                "state=unavailable reasonKind=asp-python-graphs-artifact-digest-mismatch expected={} observed={observed}",
                self.descriptor.content_digest
            ));
        }
        let mut command = tokio::process::Command::new(&self.descriptor.execution_artifact_locator);
        let mut placeholder_count = 0usize;
        for argument in &self.descriptor.command_arguments {
            placeholder_count += argument.matches("${socketPath}").count();
            if argument.contains("${") && !argument.contains("${socketPath}") {
                return Err(
                    "asp-python-graphs artifact command contains unresolved placeholder".to_owned(),
                );
            }
            command.arg(argument.replace("${socketPath}", &socket_path.to_string_lossy()));
        }
        if placeholder_count != 1 {
            return Err(
                "asp-python-graphs artifact command requires exactly one socketPath placeholder"
                    .to_owned(),
            );
        }
        command.env_clear();
        command.kill_on_drop(true);
        Ok(command)
    }
}

fn valid_digest(value: &str) -> bool {
    value.starts_with("blake3-256:")
        && value.len() == 75
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
            protocol_version: PROTOCOL_VERSION.to_owned(),
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
                "protocolVersion": PROTOCOL_VERSION,
                "executionArtifactLocator": &path,
                "commandArguments": ["--socket=${socketPath}"],
                "contentDigest": digest(content),
                "executionCommandDigest": digest_json(&serde_json::json!({
                    "executable": path,
                    "arguments": ["--socket=${socketPath}"],
                })),
                "publicationGeneration": 1,
                "publicationNonce": "test-publication-1",
            })),
            publication_generation: 1,
            publication_nonce: "test-publication-1".to_owned(),
        }
    }

    #[tokio::test]
    async fn descriptor_verifies_absolute_entrypoint_and_content_digest() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("asp-python-graphs");
        let content = b"immutable-entrypoint";
        tokio::fs::write(&path, content).await.expect("entrypoint");
        let verified = descriptor(path, content)
            .verify()
            .await
            .expect("verified descriptor");
        let command = verified
            .command_for_socket(&directory.path().join("graphs.sock"))
            .expect("command");
        assert_eq!(
            command.as_std().get_program(),
            directory.path().join("asp-python-graphs")
        );
    }

    #[tokio::test]
    async fn descriptor_digest_mismatch_is_typed_unavailable() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("asp-python-graphs");
        tokio::fs::write(&path, b"actual")
            .await
            .expect("entrypoint");
        let error = descriptor(path, b"expected")
            .verify()
            .await
            .expect_err("digest mismatch");
        assert!(error.contains("artifact-digest-mismatch"));
    }

    #[tokio::test]
    async fn publisher_atomically_commits_monotonic_reader_verified_descriptors() {
        let state_home = tempfile::tempdir().expect("state home");
        let executable = state_home.path().join("asp-python-graphs");
        tokio::fs::write(&executable, b"immutable-entrypoint")
            .await
            .expect("entrypoint");
        let first = publish_asp_python_graphs_artifact(
            state_home.path(),
            &executable,
            vec!["serve".to_owned(), "--socket=${socketPath}".to_owned()],
            Some(0),
        )
        .await
        .expect("first publication");
        let second = publish_asp_python_graphs_artifact(
            state_home.path(),
            &executable,
            vec!["serve".to_owned(), "--socket=${socketPath}".to_owned()],
            Some(first.publication_generation),
        )
        .await
        .expect("second publication");
        assert_eq!(first.publication_generation, 1);
        assert_eq!(second.publication_generation, 2);
        let verified = AspPythonGraphsArtifactDescriptor::load(&second.path)
            .await
            .expect("reader accepts writer output");
        assert_eq!(verified.descriptor().publication_generation, 2);
        assert_ne!(first.publication_nonce, second.publication_nonce);
    }

    #[tokio::test]
    async fn stale_writer_and_torn_descriptor_fail_without_replacing_authority() {
        let state_home = tempfile::tempdir().expect("state home");
        let executable = state_home.path().join("asp-python-graphs");
        tokio::fs::write(&executable, b"immutable-entrypoint")
            .await
            .expect("entrypoint");
        let first = publish_asp_python_graphs_artifact(
            state_home.path(),
            &executable,
            vec!["serve".to_owned(), "--socket=${socketPath}".to_owned()],
            Some(0),
        )
        .await
        .expect("first publication");
        let stale = publish_asp_python_graphs_artifact(
            state_home.path(),
            &executable,
            vec!["serve".to_owned(), "--socket=${socketPath}".to_owned()],
            Some(0),
        )
        .await
        .expect_err("stale publication must fail");
        assert!(stale.contains("asp-python-graphs-stale-writer"));
        let still_current = AspPythonGraphsArtifactDescriptor::load(&first.path)
            .await
            .expect("stale writer preserved current descriptor");
        assert_eq!(still_current.descriptor().publication_generation, 1);

        tokio::fs::write(&first.path, b"{\"schemaId\":")
            .await
            .expect("torn descriptor fixture");
        let torn = AspPythonGraphsArtifactDescriptor::load(&first.path)
            .await
            .expect_err("torn descriptor must fail");
        assert!(torn.contains("descriptor decode failed"));
        let repair = publish_asp_python_graphs_artifact(
            state_home.path(),
            &executable,
            vec!["serve".to_owned(), "--socket=${socketPath}".to_owned()],
            None,
        )
        .await
        .expect_err("publisher must not overwrite an undecodable authority");
        assert!(repair.contains("existing-descriptor-invalid"));
    }

    #[tokio::test]
    async fn publisher_shares_the_runtime_artifact_mutation_authority() {
        let state_home = tempfile::tempdir().expect("state home");
        let executable = state_home.path().join("asp-python-graphs");
        tokio::fs::write(&executable, b"immutable-entrypoint")
            .await
            .expect("entrypoint");
        let artifact_root = state_home.path().join("runtime/artifacts");
        std::fs::create_dir_all(&artifact_root).expect("artifact root");
        let _guard = agent_semantic_artifacts::runtime_artifact_retention::RuntimeArtifactMutationGuard::try_acquire(
            &artifact_root,
        )
        .expect("hold shared artifact mutation authority");
        let error = publish_asp_python_graphs_artifact(
            state_home.path(),
            &executable,
            vec!["serve".to_owned(), "--socket=${socketPath}".to_owned()],
            Some(0),
        )
        .await
        .expect_err("concurrent artifact writer must fail closed");
        assert!(error.contains("reasonKind=artifact-publication-conflict"));
    }
}
