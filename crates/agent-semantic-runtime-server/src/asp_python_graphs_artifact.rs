//! Active-bundle authority for the optional resident Python Graphs service.
//!
//! The Python worker is an optional Runtime capability, not a Search generation
//! publisher. Its executable is admitted only as a content-proven member of
//! the active immutable Runtime bundle. There is intentionally no state-home
//! descriptor, generation counter, public publisher, or PATH fallback.

use std::path::Path;
use std::path::PathBuf;

use agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest;
use agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactSlotAuthority;
use agent_semantic_artifacts::runtime_artifact_slots::verify_runtime_artifact_bundle;

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
        let resident_root = agent_semantic_artifacts::RuntimeArtifactStateLayout::new(state_home)
            .root()
            .to_path_buf();
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
#[path = "../tests/unit/asp_python_graphs_artifact.rs"]
mod tests;
