//! Active-bundle authority for the optional resident Python Graphs service.
//!
//! The Python worker is an optional Runtime capability, not a Search generation
//! publisher. Its executable is admitted only as a content-proven member of
//! the active immutable Runtime bundle. There is intentionally no state-home
//! descriptor, generation counter, public publisher, or PATH fallback.

use std::path::{Path, PathBuf};

use agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest;
use agent_semantic_artifacts::runtime_artifact_slots::{
    RuntimeArtifactSlotAuthority, verify_runtime_artifact_bundle,
};

pub const ASP_PYTHON_GRAPHS_BUNDLE_MEMBER: &str = "asp-python-graphs";
pub const PROTOCOL_NAMESPACE: &str = "asp.python.graphs";
pub const PROTOCOL_VERSION: &str = "1";

#[derive(Clone, Debug)]
pub struct VerifiedAspPythonGraphsArtifact {
    executable: PathBuf,
    content_digest: Blake3ContentDigest,
    bundle_digest: Blake3ContentDigest,
    execution_command_digest: String,
}

impl VerifiedAspPythonGraphsArtifact {
    /// Resolve and verify the optional worker from the exact active Runtime
    /// bundle. The immutable candidate target is retained after validation so
    /// a later active-slot switch cannot change this process identity.
    pub async fn load_from_active_runtime_bundle(state_home: &Path) -> Result<Self, String> {
        let resident_root = state_home.join("runtime/resident");
        let slots = RuntimeArtifactSlotAuthority::new(&resident_root);
        let active = slots.active_target().await?.ok_or_else(|| {
            "state=unavailable reasonKind=asp-python-graphs-runtime-bundle-not-active".to_owned()
        })?;
        let bundle = verify_runtime_artifact_bundle(&active).await?;
        let executable = bundle
            .member_path(ASP_PYTHON_GRAPHS_BUNDLE_MEMBER)
            .ok_or_else(|| {
                "state=unavailable reasonKind=asp-python-graphs-bundle-member-not-installed"
                    .to_owned()
            })?;
        let content_digest = bundle
            .member_digest(ASP_PYTHON_GRAPHS_BUNDLE_MEMBER)
            .cloned()
            .ok_or_else(|| {
                "state=unavailable reasonKind=asp-python-graphs-bundle-member-not-installed"
                    .to_owned()
            })?;
        let bundle_digest = bundle.bundle_digest().clone();
        let execution_command_digest = digest_json(&serde_json::json!({
            "bundleDigest": &bundle_digest,
            "member": ASP_PYTHON_GRAPHS_BUNDLE_MEMBER,
            "contentDigest": &content_digest,
            "arguments": ["--socket=${socketPath}"],
        }));
        Ok(Self {
            executable,
            content_digest,
            bundle_digest,
            execution_command_digest,
        })
    }

    pub fn content_digest(&self) -> &str {
        self.content_digest.as_str()
    }

    pub fn bundle_digest(&self) -> &str {
        self.bundle_digest.as_str()
    }

    pub fn execution_command_digest(&self) -> &str {
        &self.execution_command_digest
    }

    pub fn command_for_socket(
        &self,
        socket_path: &Path,
    ) -> Result<tokio::process::Command, String> {
        if !socket_path.is_absolute() {
            return Err("asp-python-graphs socket locator must be absolute".to_owned());
        }
        let bytes = std::fs::read(&self.executable).map_err(|error| {
            format!(
                "state=unavailable reasonKind=asp-python-graphs-bundle-member-read-failed error={error}"
            )
        })?;
        let observed = Blake3ContentDigest::from_bytes(&bytes);
        if observed != self.content_digest {
            return Err(format!(
                "state=unavailable reasonKind=asp-python-graphs-bundle-member-digest-mismatch expected={} observed={observed}",
                self.content_digest
            ));
        }
        let mut command = tokio::process::Command::new(&self.executable);
        command.arg(format!("--socket={}", socket_path.to_string_lossy()));
        command.env_clear();
        command.kill_on_drop(true);
        Ok(command)
    }
}

fn digest_json(value: &serde_json::Value) -> String {
    let bytes = serde_json::to_vec(value).expect("artifact identity is serializable");
    format!("blake3-256:{}", blake3::hash(&bytes).to_hex())
}

#[cfg(all(test, unix))]
mod tests {
    use std::collections::BTreeMap;
    use std::os::unix::fs::{PermissionsExt, symlink};

    use agent_semantic_artifacts::runtime_artifact_slots::runtime_artifact_bundle_digest;

    use super::*;

    fn publish_bundle_fixture(state_home: &Path, members: &[(&str, &[u8])]) -> PathBuf {
        let candidate = state_home.join("runtime/resident/candidates/asp/test-bundle");
        std::fs::create_dir_all(&candidate).expect("candidate directory");
        let mut digests = BTreeMap::new();
        for (name, bytes) in members {
            let path = candidate.join(name);
            std::fs::write(&path, bytes).expect("member bytes");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("member executable permission");
            digests.insert((*name).to_owned(), Blake3ContentDigest::from_bytes(bytes));
        }
        let bundle_digest = runtime_artifact_bundle_digest(&digests);
        std::fs::write(
            candidate.join("bundle.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-binary-bundle",
                "schemaVersion": 1,
                "bundleDigest": bundle_digest,
                "members": digests,
            }))
            .expect("bundle manifest"),
        )
        .expect("write bundle manifest");
        let active = state_home.join("runtime/resident/active");
        std::fs::create_dir_all(active.parent().expect("active parent"))
            .expect("resident directory");
        symlink(&candidate, &active).expect("active selector");
        candidate
    }

    #[tokio::test]
    async fn exact_active_bundle_member_is_the_only_graph_worker_authority() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let state_home = temporary.path();
        let candidate = publish_bundle_fixture(
            state_home,
            &[
                ("asp", b"asp"),
                (ASP_PYTHON_GRAPHS_BUNDLE_MEMBER, b"graphs"),
            ],
        );

        let artifact = VerifiedAspPythonGraphsArtifact::load_from_active_runtime_bundle(state_home)
            .await
            .expect("verified graph member");

        assert_eq!(
            artifact.executable,
            candidate.join(ASP_PYTHON_GRAPHS_BUNDLE_MEMBER)
        );
        assert_eq!(
            artifact.content_digest,
            Blake3ContentDigest::from_bytes(b"graphs")
        );
    }

    #[tokio::test]
    async fn missing_graph_member_is_typed_capability_unavailable() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        publish_bundle_fixture(temporary.path(), &[("asp", b"asp")]);

        let error =
            VerifiedAspPythonGraphsArtifact::load_from_active_runtime_bundle(temporary.path())
                .await
                .expect_err("missing optional member must fail closed");

        assert!(error.contains("reasonKind=asp-python-graphs-bundle-member-not-installed"));
    }

    #[tokio::test]
    async fn graph_member_digest_drift_fails_before_process_construction() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let candidate = publish_bundle_fixture(
            temporary.path(),
            &[
                ("asp", b"asp"),
                (ASP_PYTHON_GRAPHS_BUNDLE_MEMBER, b"graphs"),
            ],
        );
        std::fs::write(candidate.join(ASP_PYTHON_GRAPHS_BUNDLE_MEMBER), b"drift")
            .expect("corrupt graph member");

        let error =
            VerifiedAspPythonGraphsArtifact::load_from_active_runtime_bundle(temporary.path())
                .await
                .expect_err("digest drift must fail closed");

        assert!(error.contains("Runtime artifact bundle member digest mismatch"));
    }
}
