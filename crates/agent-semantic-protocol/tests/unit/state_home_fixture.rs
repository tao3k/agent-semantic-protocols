//! Shared State Home v1 fixtures for protocol integration tests.

use std::path::{Path, PathBuf};

use agent_semantic_hook::{
    ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy, HookActivation,
    active_provider_artifact_input_with_state_home, builtin_provider_manifests,
    materialize_provider_routes, provider_execution_command_digest, provider_manifest_digest,
    semantic_registry_digest,
};
use agent_semantic_runtime::state_core::ResolvedState;

pub(crate) fn canonical_activation_path(root: &Path, state_home: &Path) -> PathBuf {
    resolved_state(root, state_home)
        .paths
        .hooks_dir
        .join("state/activation.json")
}

pub(crate) fn install_provider_script(
    state_home: &Path,
    language_id: &str,
    script: &str,
) -> PathBuf {
    let manifest = manifest_for(language_id);
    let provider_path = state_home.join("runtime/bin").join(manifest.binary());
    std::fs::create_dir_all(provider_path.parent().expect("provider runtime bin parent"))
        .expect("create State Home provider runtime bin");
    std::fs::write(&provider_path, script).expect("write State Home provider");
    make_executable(&provider_path);
    let provider_path =
        std::fs::canonicalize(&provider_path).expect("canonical State Home provider path");
    write_provider_lock(state_home, &manifest, &provider_path);
    provider_path
}

pub(crate) fn write_activation(root: &Path, state_home: &Path, language_ids: &[&str]) -> PathBuf {
    let resolved = resolved_state(root, state_home);
    resolved
        .ensure_minimal_layout()
        .expect("materialize canonical State Home layout");
    agent_semantic_client_db::AgentSessionRegistry::open_or_create_state_root(&resolved.state_home)
        .expect("materialize canonical agent-session registry");
    let canonical_root = std::fs::canonicalize(root).expect("canonical protocol fixture root");
    let semantic_registry_digest = semantic_registry_digest();
    let providers = language_ids
        .iter()
        .map(|language_id| {
            let manifest = manifest_for(language_id);
            let provider_path = state_home.join("runtime/bin").join(manifest.binary());
            let provider_path = std::fs::canonicalize(&provider_path).unwrap_or_else(|error| {
                panic!(
                    "State Home provider must be installed before activation: {}: {error}",
                    provider_path.display()
                )
            });
            write_provider_lock(state_home, &manifest, &provider_path);
            let artifact = active_provider_artifact_input_with_state_home(
                &canonical_root,
                state_home,
                manifest.language_id(),
                manifest.provider_id(),
                provider_path.clone(),
            )
            .expect("validate provider artifact against typed install receipt");
            let resolved_execution_prefix = vec![provider_path.display().to_string()];
            ActivatedProviderConfig {
                manifest_id: manifest.manifest_id().to_string(),
                manifest_digest: provider_manifest_digest(&manifest)
                    .expect("provider manifest digest"),
                language_id: manifest.language_id().clone(),
                provider_id: manifest.provider_id().clone(),
                binary: manifest.binary().to_string(),
                execution: manifest.execution(),
                execution_command_digest: provider_execution_command_digest(
                    &resolved_execution_prefix,
                    &artifact.artifact_digest,
                )
                .expect("provider execution command digest"),
                provider_command_prefix: Vec::new(),
                search_capabilities: manifest.search_capabilities().clone(),
                semantic_facts_descriptor: manifest.semantic_facts_descriptor().cloned(),
                query_pack_descriptor: manifest.query_pack_descriptor().clone(),
                semantic_registry_digest: semantic_registry_digest.clone(),
                routes: materialize_provider_routes(&manifest).expect("provider routes"),
                coverage: ActivationCoverage {
                    package_roots: vec![canonical_root.display().to_string()],
                    config_files: crate::provider_manifest_scope::project_entries(&manifest),
                    source_extensions: crate::provider_manifest_scope::document_extensions(
                        &manifest,
                    ),
                },
            }
        })
        .collect();
    let activation = HookActivation {
        rankers: Vec::new(),
        schema_id: agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_ID.to_string(),
        schema_version: agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_VERSION.to_string(),
        schema_authority: agent_semantic_hook::CANONICAL_SCHEMA_AUTHORITY.to_string(),
        protocol_id: agent_semantic_hook::HOOK_PROTOCOL_ID.to_string(),
        protocol_version: agent_semantic_hook::HOOK_PROTOCOL_VERSION.to_string(),
        project_root: canonical_root.display().to_string(),
        generated_by: ActivationGeneratedBy {
            runtime: "asp".to_string(),
            version: "test".to_string(),
        },
        generated_at: None,
        providers,
    };
    let activation_path = canonical_activation_path(root, state_home);
    std::fs::create_dir_all(activation_path.parent().expect("activation parent"))
        .expect("create canonical hook state");
    std::fs::write(
        &activation_path,
        serde_json::to_vec_pretty(&activation).expect("serialize typed hook activation"),
    )
    .expect("write canonical hook activation");
    activation_path
}

fn resolved_state(root: &Path, state_home: &Path) -> ResolvedState {
    ResolvedState::resolve_with_state_home(root, state_home)
        .expect("resolve protocol fixture State")
}

fn manifest_for(language_id: &str) -> agent_semantic_hook::ProviderManifest {
    builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id().as_str() == language_id)
        .unwrap_or_else(|| panic!("missing built-in provider manifest for {language_id}"))
}

fn write_provider_lock(
    state_home: &Path,
    manifest: &agent_semantic_hook::ProviderManifest,
    provider_path: &Path,
) {
    let entrypoint_digest = agent_semantic_content_identity::file_content_digest_v1(provider_path)
        .expect("installed provider content digest");
    let metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(provider_path)
            .expect("installed provider metadata digest");
    let lock_dir = agent_semantic_runtime::provider_receipt_dir(&state_home);
    std::fs::create_dir_all(&lock_dir).expect("create State Home provider lock registry");
    std::fs::write(
        lock_dir.join(format!("{}.lock.toml", manifest.language_id())),
        format!(
            "schemaId = \"asp.provider-install-lock.v1\"\nlanguage = \"{}\"\nprovider = \"{}\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{}\"\ninstalledEntrypointMetadataDigest = \"{}\"\n",
            manifest.language_id(),
            manifest.provider_id(),
            provider_path.display(),
            entrypoint_digest,
            metadata_digest,
        ),
    )
    .expect("write typed provider lock receipt");
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .expect("provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("provider permissions");
    }
    #[cfg(not(unix))]
    let _ = path;
}
