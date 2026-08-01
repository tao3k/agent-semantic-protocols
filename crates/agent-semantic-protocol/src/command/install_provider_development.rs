//! Development-checkout provider build delegation.

use std::env;
use std::path::{Path, PathBuf};

use agent_semantic_hook::ProviderDevelopmentArtifactDomain;

pub(super) struct DevelopmentArtifactProvenance {
    pub source_snapshot_root: String,
    pub source_leaf_count: usize,
    pub provider_digest: String,
    pub build_recipe_digest: String,
}

pub(super) fn development_artifact_is_authorized(
    provider_source_root: &Path,
    state_home: &Path,
    registered_binary: &str,
    artifact_domain: ProviderDevelopmentArtifactDomain,
    artifact: &Path,
) -> bool {
    match artifact_domain {
        ProviderDevelopmentArtifactDomain::Checkout => artifact.starts_with(provider_source_root),
        ProviderDevelopmentArtifactDomain::StateHomeProviderStaging => artifact.starts_with(
            state_home
                .join("runtime/provider-artifacts")
                .join(registered_binary)
                .join("develop"),
        ),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct DevelopmentProviderInstallerPlan {
    root: PathBuf,
    args: Vec<String>,
}

fn development_provider_installer_plan(
    configured_dev_root: &Path,
    language_id: &str,
    target: &str,
    project_root: Option<&Path>,
) -> Result<DevelopmentProviderInstallerPlan, String> {
    let root = configured_dev_root.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize configured [dev].root {}: {error}",
            configured_dev_root.display()
        )
    })?;
    let justfile = root.join("Justfile");
    if !justfile.is_file() {
        return Err(format!(
            "configured [dev].root has no canonical development installer: {}",
            justfile.display()
        ));
    }
    let (scope, project) = project_root.map_or_else(
        || ("global", String::new()),
        |project| ("project", project.display().to_string()),
    );
    Ok(DevelopmentProviderInstallerPlan {
        root: root.clone(),
        args: vec![
            "exec".to_string(),
            root.display().to_string(),
            "just".to_string(),
            "--justfile".to_string(),
            justfile.display().to_string(),
            "agent-tools-install-language".to_string(),
            language_id.to_string(),
            target.to_string(),
            scope.to_string(),
            project,
        ],
    })
}

pub(super) fn run_development_provider_installer(
    configured_dev_root: &Path,
    language_id: &str,
    target: &str,
    project_root: Option<&Path>,
) -> Result<(), String> {
    if env::var_os("ASP_DEV_INSTALL_DELEGATED").is_some() {
        return Err(
            "development provider installer recursively re-entered without publishing an artifact receipt"
                .to_string(),
        );
    }
    let plan = development_provider_installer_plan(
        configured_dev_root,
        language_id,
        target,
        project_root,
    )?;
    let status = agent_semantic_runtime::runtime_block_on_current_thread(async {
        tokio::process::Command::new("direnv")
            .args(&plan.args)
            .current_dir(&plan.root)
            .env("ASP_DEV_INSTALL_DELEGATED", "1")
            .status()
            .await
    })?
    .map_err(|error| {
        format!(
            "failed to start development provider installer at {}: {error}",
            plan.root.display()
        )
    })?;
    if !status.success() {
        return Err(format!(
            "development provider installer failed: language={language_id} devRoot={} status={status}",
            plan.root.display()
        ));
    }
    Ok(())
}

pub(super) fn capture_development_artifact_provenance(
    dev_root: &Path,
    registration: &agent_semantic_hook::ProviderDevelopmentRegistrationV1,
    target: &str,
) -> Result<DevelopmentArtifactProvenance, String> {
    let provider_source_root = dev_root
        .join(&registration.development.source_root)
        .canonicalize()
        .map_err(|error| {
            format!(
                "canonicalize provider development sourceRoot {}: {error}",
                registration.development.source_root
            )
        })?;
    if !provider_source_root.starts_with(dev_root) {
        return Err(format!(
            "provider development sourceRoot escaped [dev].root: language={} sourceRoot={}",
            registration.language_id.as_str(),
            provider_source_root.display()
        ));
    }
    let snapshot =
        agent_semantic_runtime::git::discover_repository_candidate_snapshot(&provider_source_root)
            .map_err(|error| format!("capture provider source candidate scope: {error}"))?
            .ok_or_else(|| {
                format!(
                    "provider development sourceRoot is not a Git worktree: {}",
                    provider_source_root.display()
                )
            })?;
    let file_hashes = snapshot
        .candidates
        .iter()
        .filter_map(|candidate| {
            let path = provider_source_root.join(&candidate.path);
            path.is_file().then_some((candidate.path.as_path(), path))
        })
        .map(|(relative_path, path)| {
            agent_semantic_content_identity::file_content_digest_v1(&path)
                .map(|digest| (relative_path.to_string_lossy().into_owned(), digest))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let source_leaf_count = file_hashes.len();
    let source_snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(file_hashes);
    let provider_digest = agent_semantic_hook::registered_provider_catalog_identities()
        .iter()
        .find(|identity| identity.language_id == registration.language_id.as_str())
        .map(|identity| identity.manifest_digest.clone())
        .ok_or_else(|| {
            format!(
                "no ProviderRegistry catalog identity for `{}`",
                registration.language_id.as_str()
            )
        })?;
    let justfile_digest =
        agent_semantic_content_identity::file_content_digest_v1(&dev_root.join("Justfile"))?;
    let build_recipe_digest = format!(
        "blake3-256:{}",
        blake3::hash(
            format!(
                "{}\0{}\0{}\0{}\0{}",
                registration.development.build_binding,
                registration.language_id.as_str(),
                registration.development.source_root,
                target,
                justfile_digest
            )
            .as_bytes()
        )
        .to_hex()
    );
    Ok(DevelopmentArtifactProvenance {
        source_snapshot_root: source_snapshot.root_digest().to_string(),
        source_leaf_count,
        provider_digest,
        build_recipe_digest,
    })
}

#[cfg(test)]
#[path = "../../tests/unit/install_provider_development.rs"]
mod tests;
