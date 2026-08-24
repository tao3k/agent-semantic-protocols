use std::path::Path;

use agent_semantic_provider_protocol::{
    ProviderRegisterOperation, ProviderRegisterRequest, ProviderRegisterResult,
    ProviderRegistrationDocument,
};
use agent_semantic_runtime::{ensure_project_provider_lock_dir, project_runtime_state};

use super::BuiltProviderWorkspace;

#[allow(clippy::too_many_arguments)]
pub(in super::super) async fn record_registered_provider_workspace_install(
    language_id: &str,
    provider_id: &str,
    registered_binary: &str,
    target: &str,
    invocation_root: &Path,
    project_root: Option<&Path>,
    install_scope: &super::super::InstallScope,
    configured_dev_root: &Path,
    registration: &super::super::super::provider_install_registry::ProviderInstallRegistration,
    built: BuiltProviderWorkspace,
) -> Result<(), String> {
    let live_registration = built.provider_registration.clone();
    let dev_root = configured_dev_root.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize configured [dev].root {}: {error}",
            configured_dev_root.display()
        )
    })?;
    validate_registration_identity(language_id, provider_id, registered_binary, registration)?;
    let provider_source_root = dev_root
        .join(&registration.source_root)
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize provider sourceRoot: {error}"))?;
    let runtime_state = project_runtime_state(project_root.unwrap_or(invocation_root))?;
    let reconciliation_guard =
        super::super::super::protocol_binary::ProtocolBinaryReconciliationGuard::acquire(
            &runtime_state.protocol_home,
        )?;
    let provider_binary = super::super::binary_file_name(registered_binary, target);
    let install_target =
        super::super::resolve_provider_binary_install_target(language_id, &provider_binary)?;
    let stable_entry = project_root.map_or_else(
        || install_target.path.clone(),
        |_| runtime_state.runtime_bin_dir.join(&provider_binary),
    );
    let binary_artifact_root = runtime_state.protocol_home.join("runtime/artifacts");
    let published = super::publish_provider_workspace(
        &runtime_state.protocol_home,
        &stable_entry,
        &binary_artifact_root,
        registration,
        built,
        &reconciliation_guard,
    )
    .await?;
    let installed_entrypoint_digest =
        agent_semantic_content_identity::file_content_digest_v1(&published.installed_path)?;
    let installed_entrypoint_metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(
            &published.installed_path,
        )?;
    let execution_command_digest = agent_semantic_hook::provider_execution_command_digest(
        &[published.installed_path.to_string_lossy().to_string()],
        &installed_entrypoint_digest,
    )?;
    let provenance =
        super::super::capture_development_artifact_provenance(&dev_root, registration, target)?;
    let installed_sha256 = super::super::sha256_file(&published.installed_path)?;
    let artifact_entrypoint_sha256 = super::super::sha256_file(&published.artifact_entrypoint)?;
    let launcher_digest =
        agent_semantic_content_identity::file_content_digest_v1(&published.launcher)?;
    let (scope, lock_path) = install_scope_and_lock(language_id, install_scope)?;
    super::super::write_provider_lock(
        &lock_path,
        &super::super::ProviderInstallLock {
            schema_id: "asp.provider-install-lock.v1",
            scope,
            language_id,
            provider_id,
            source_kind: "develop-workspace-tree",
            checkout_root: Some(&dev_root),
            provider_source_root: Some(&provider_source_root),
            repo: None,
            rev: None,
            target,
            binary: registered_binary,
            installed_path: &published.installed_path,
            package_path: &published.artifact_root,
            sha256: &installed_sha256,
            source: published.source_root.display().to_string(),
            source_snapshot_root: Some(&provenance.source_snapshot_root),
            source_snapshot_algorithm: Some("blake3-merkle-v1"),
            source_leaf_count: Some(provenance.source_leaf_count),
            provider_digest: Some(&provenance.provider_digest),
            build_recipe_digest: Some(&provenance.build_recipe_digest),
            artifact_digest: Some(&published.artifact_digest),
            artifact_leaf_count: Some(published.artifact_leaf_count),
            artifact_entrypoint: Some(&published.artifact_entrypoint),
            artifact_entrypoint_sha256: Some(&artifact_entrypoint_sha256),
            installed_entrypoint_digest: Some(&installed_entrypoint_digest),
            installed_entrypoint_metadata_digest: &installed_entrypoint_metadata_digest,
            execution_command_digest: &execution_command_digest,
            launcher_digest: Some(&launcher_digest),
        },
    )?;
    publish_live_provider_registration(&runtime_state.protocol_home, live_registration).await?;
    let installed_provider_artifacts = publish_installed_artifacts_if_needed(
        &runtime_state.protocol_home,
        install_scope,
        &reconciliation_guard,
    )?;
    drop(reconciliation_guard);
    println!(
        "[asp-install] provider={} language={} scope={} installMode=develop-workspace-tree sourceKind=develop-workspace-tree devRoot={} target={} binary={} binaryContentDigest={} digestAlgorithm=blake3-256 artifactLeafCount={} artifactEntrypoint={} installedPath={} lock={} switch=atomic installedProviderArtifacts={} installedProviderArtifactsWrite={} installedProviderArtifactsChangedLeaves={} installedProviderArtifactsElapsedMicros={}",
        provider_id,
        language_id,
        scope,
        dev_root.display(),
        target,
        registered_binary,
        published.artifact_digest,
        published.artifact_leaf_count,
        published.artifact_entrypoint.display(),
        published.installed_path.display(),
        lock_path.display(),
        installed_provider_artifacts
            .as_ref()
            .map(|publication| publication.generation.as_str())
            .unwrap_or("not-applicable"),
        installed_provider_artifacts
            .as_ref()
            .is_some_and(|publication| publication.artifact_write),
        installed_provider_artifacts
            .as_ref()
            .map_or(0, |publication| publication.changed_leaf_count),
        installed_provider_artifacts
            .as_ref()
            .map_or(0, |publication| publication.elapsed_micros),
    );
    Ok(())
}

async fn publish_live_provider_registration(
    state_home: &Path,
    provider: ProviderRegistrationDocument,
) -> Result<(), String> {
    let endpoint =
        agent_semantic_client_db::runtime_server_control::read_runtime_server_supervisor_endpoint(
            state_home,
        )
        .await?
        .ok_or_else(|| {
            "Runtime Server endpoint is unavailable for provider registration".to_owned()
        })?;
    for _ in 0..3 {
        let list = ProviderRegisterRequest {
            schema_id: "agent.semantic-protocols.provider-register.request".to_owned(),
            schema_version: "1".to_owned(),
            expected_generation: None,
            request: ProviderRegisterOperation::List,
        };
        let list = agent_semantic_provider_transport::grpc_session::call_runtime_provider_register(
            &endpoint.provider_plane_socket_path,
            &list,
        )
        .await?;
        list.validate()?;
        let generation = match list.result {
            ProviderRegisterResult::Snapshot { snapshot } => snapshot.generation,
            ProviderRegisterResult::GenerationConflict { actual_generation } => actual_generation,
            ProviderRegisterResult::Rejected {
                reason_kind,
                message,
            } => {
                return Err(format!(
                    "Runtime provider register list rejected: reasonKind={reason_kind} message={message}"
                ));
            }
        };
        let register = ProviderRegisterRequest {
            schema_id: "agent.semantic-protocols.provider-register.request".to_owned(),
            schema_version: "1".to_owned(),
            expected_generation: Some(generation),
            request: ProviderRegisterOperation::Register {
                provider: provider.clone(),
            },
        };
        let response =
            agent_semantic_provider_transport::grpc_session::call_runtime_provider_register(
                &endpoint.provider_plane_socket_path,
                &register,
            )
            .await?;
        response.validate()?;
        match response.result {
            ProviderRegisterResult::Snapshot { .. } => return Ok(()),
            ProviderRegisterResult::GenerationConflict { .. } => continue,
            ProviderRegisterResult::Rejected {
                reason_kind,
                message,
            } => {
                return Err(format!(
                    "Runtime provider registration rejected: reasonKind={reason_kind} message={message}"
                ));
            }
        }
    }
    Err("Runtime provider registration generation remained contended after 3 attempts".to_owned())
}

fn validate_registration_identity(
    language_id: &str,
    provider_id: &str,
    registered_binary: &str,
    registration: &super::super::super::provider_install_registry::ProviderInstallRegistration,
) -> Result<(), String> {
    if registration.provider_id == provider_id
        && registration.binary == registered_binary
        && registration.language_id == language_id
    {
        return Ok(());
    }
    Err(format!(
        "ProviderRegistry workspace-install identity drift: language={} provider={} binary={} expectedLanguage={language_id} expectedProvider={provider_id} expectedBinary={registered_binary}",
        registration.language_id, registration.provider_id, registration.binary
    ))
}

fn install_scope_and_lock<'a>(
    language_id: &str,
    install_scope: &'a super::super::InstallScope,
) -> Result<(&'a str, std::path::PathBuf), String> {
    match install_scope {
        super::super::InstallScope::Global => Ok((
            "global",
            super::super::canonical_global_provider_state_root()?
                .join("receipts")
                .join(format!("{language_id}.lock.toml")),
        )),
        super::super::InstallScope::Project { root } => Ok((
            "project",
            ensure_project_provider_lock_dir(root)?.join(format!("{language_id}.lock.toml")),
        )),
    }
}

fn publish_installed_artifacts_if_needed(
    state_home: &Path,
    install_scope: &super::super::InstallScope,
    reconciliation_guard: &super::super::super::protocol_binary::ProtocolBinaryReconciliationGuard,
) -> Result<
    Option<crate::command::installed_provider_artifacts::InstalledProviderArtifactsPublication>,
    String,
> {
    if !matches!(install_scope, super::super::InstallScope::Global) {
        return Ok(None);
    }
    super::super::super::installed_provider_artifacts::publish_current_installed_provider_artifacts(
        state_home,
        reconciliation_guard,
    )
    .map(Some)
}
