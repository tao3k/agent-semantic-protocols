use std::path::{Path, PathBuf};

use crate::protocol_activation::digest::provider_manifest_digest;
use crate::protocol_activation::protocol_activation_manifest::{
    ActivatedProvider, ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy,
    HookActivation, HookRuntime,
};
use crate::provider_manifest::provider_manifests;

use super::{
    RuntimeProviderHealthStatus, runtime_profile_invocation, runtime_profiles_for_activation,
    runtime_profiles_for_runtime, runtime_project_root_for_activation, runtime_provider_command,
};

#[test]
fn runtime_project_root_for_generated_activation_uses_activation_storage_root() {
    let root = temp_root("activation-project-root");
    std::fs::create_dir_all(root.join(".git")).expect("git marker");
    let state_home = root.join("state-home");
    let resolved = agent_semantic_runtime::state_core::ResolvedState::resolve_with_state_home(
        &root, state_home,
    )
    .expect("resolve State Home activation path");
    std::fs::create_dir_all(&resolved.paths.workspace_dir).expect("workspace state");
    let canonical_root = std::fs::canonicalize(&root).expect("canonical project root");
    std::fs::write(
        &resolved.paths.workspace_json,
        serde_json::to_string(&serde_json::json!({
            "root": canonical_root.display().to_string()
        }))
        .expect("workspace manifest"),
    )
    .expect("write workspace manifest");
    let activation_path = resolved.paths.hooks_dir.join("state/activation.json");

    assert_eq!(
        runtime_project_root_for_activation(&activation_path, "."),
        canonical_root
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn resolved_provider_binary_materializes_authoritative_command() {
    let root = temp_root("resolved-provider");
    let resolved = write_executable_provider(&root, "rs-harness");
    let prefix = vec![resolved.display().to_string()];
    let resolution = crate::executable::resolve_executable_with_status(&prefix[0]);

    let command = runtime_provider_command(&prefix, &resolution);

    assert_eq!(
        command.argv,
        [std::fs::canonicalize(&resolved)
            .unwrap_or(resolved)
            .display()
            .to_string()]
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn runtime_profiles_for_activation_rejects_persisted_command_prefix() {
    let root = temp_root("activation-command-prefix");
    let wrapper = write_executable_provider(&root, "provider-wrapper");
    let provider = activated_rust_provider(vec![
        wrapper.display().to_string(),
        "rs-harness".to_string(),
    ]);
    let activation = HookActivation {
        rankers: Vec::new(),
        schema_id: crate::HOOK_ACTIVATION_SCHEMA_ID.to_string(),
        schema_version: crate::HOOK_ACTIVATION_SCHEMA_VERSION.to_string(),
        schema_authority: crate::protocol::CANONICAL_SCHEMA_AUTHORITY.to_string(),
        protocol_id: crate::HOOK_PROTOCOL_ID.to_string(),
        protocol_version: crate::HOOK_PROTOCOL_VERSION.to_string(),
        project_root: root.display().to_string(),
        generated_by: ActivationGeneratedBy {
            runtime: "asp".to_string(),
            version: "test".to_string(),
        },
        generated_at: None,
        providers: vec![ActivatedProviderConfig {
            manifest_id: provider.manifest_id.clone(),
            manifest_digest: provider.manifest_digest.clone(),
            language_id: provider.language_id.clone(),
            provider_id: provider.provider_id.clone(),
            binary: provider.binary.clone(),
            execution: provider.execution,
            provider_command_prefix: provider.provider_command_prefix.clone(),
            execution_command_digest: provider.execution_command_digest.clone(),
            search_capabilities: provider.search_capabilities.clone(),
            language_projection: provider.language_projection.clone(),
            semantic_facts_descriptor: provider.semantic_facts_descriptor.clone(),
            query_pack_descriptor: provider.query_pack_descriptor.clone(),
            semantic_registry_digest: provider.semantic_registry_digest.clone(),
            routes: provider.routes.clone(),
            coverage: ActivationCoverage {
                package_roots: provider.package_roots.clone(),
                config_files: provider.config_files.clone(),
                source_extensions: provider.source_extensions.clone(),
            },
        }],
    };
    let error = runtime_profiles_for_activation(&root, &activation)
        .err()
        .expect("persisted provider command prefix must fail closed");
    assert!(
        error.contains("provider activation command prefix must be empty"),
        "unexpected activation validation error: {error}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn runtime_profiles_for_runtime_uses_activation_command_prefix() {
    let root = temp_root("gerbil-command-prefix");
    let _project_gslph = write_executable_provider(&root, "gslph");
    let wrapper = write_executable_provider(&root, "asp");
    let provider = activated_gerbil_provider(vec![
        wrapper.display().to_string(),
        "gerbil-scheme".to_string(),
    ]);
    let runtime = HookRuntime {
        rankers: Vec::new(),
        project_root: root.display().to_string(),
        providers: vec![provider],
        policy_providers: Vec::new(),
    };
    let provider = &runtime.providers[0];
    let profiles = runtime_profiles_for_runtime(&root, &runtime);
    let invocation =
        runtime_profile_invocation(&profiles, provider, &["query".into()]).expect("invocation");

    assert_eq!(
        invocation,
        [
            std::fs::canonicalize(&wrapper)
                .expect("canonical provider wrapper")
                .display()
                .to_string(),
            "gerbil-scheme".to_string(),
            "query".to_string(),
        ]
    );
    assert_eq!(
        profiles.providers[0].health.status,
        RuntimeProviderHealthStatus::Available
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn runtime_profiles_for_runtime_fails_closed_when_activation_executable_is_missing() {
    let root = temp_root("missing-activation-executable");
    let wrapper = write_executable_provider(&root, "provider-wrapper");
    let provider = activated_rust_provider(vec![
        wrapper.display().to_string(),
        "rs-harness".to_string(),
    ]);
    let runtime = HookRuntime {
        rankers: Vec::new(),
        project_root: root.display().to_string(),
        providers: vec![provider],
        policy_providers: Vec::new(),
    };
    std::fs::remove_file(&wrapper).expect("remove activated provider executable");

    let profiles = runtime_profiles_for_runtime(&root, &runtime);

    let profile = profiles
        .providers
        .first()
        .expect("runtime provider profile");
    assert_eq!(profile.health.status, RuntimeProviderHealthStatus::Missing);
    assert!(profile.resolved_binary.is_none());
    assert!(profile.argv.is_empty());
    assert!(
        profile
            .health
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("provider-wrapper")),
        "{:?}",
        profile.health.reason
    );
    assert!(
        !profile
            .argv
            .iter()
            .any(|arg| arg == &wrapper.display().to_string())
    );
    let _ = std::fs::remove_dir_all(root);
}

fn activated_rust_provider(provider_command_prefix: Vec<String>) -> ActivatedProvider {
    let manifest = provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "rust")
        .expect("rust manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let semantic_registry_digest = crate::semantic_registry_digest();
    let routes = crate::materialize_provider_routes(&manifest).expect("provider routes");
    let executable_artifact_digest =
        agent_semantic_content_identity::file_content_digest_v1(std::path::Path::new(
            provider_command_prefix
                .first()
                .expect("provider command prefix"),
        ))
        .expect("digest provider test executable");
    ActivatedProvider {
        manifest_id: manifest.manifest_id,
        manifest_digest,
        language_id: manifest.language_id,
        provider_id: manifest.provider_id,
        binary: manifest.binary,
        execution: manifest.execution,
        execution_command_digest:
            crate::protocol_activation::digest::provider_execution_command_digest(
                &provider_command_prefix,
                &executable_artifact_digest,
            )
            .expect("digest provider execution command"),
        provider_command_prefix,
        namespace: manifest.namespace,
        package_roots: vec!["src".to_string()],
        source_extensions: vec![".rs".to_string()],
        config_files: vec!["Cargo.toml".to_string()],
        search_capabilities: manifest.search_capabilities,
        language_projection: manifest.language_projection,
        project_resolution: manifest.project_resolution,
        document_resolution: manifest.document_resolution,
        semantic_facts_descriptor: manifest.semantic_facts_descriptor,
        query_pack_descriptor: manifest.query_pack_descriptor,
        semantic_registry_digest,
        policy: manifest.policy,
        routes,
    }
}

fn activated_gerbil_provider(provider_command_prefix: Vec<String>) -> ActivatedProvider {
    let manifest = provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "gerbil-scheme")
        .expect("gerbil manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let semantic_registry_digest = crate::semantic_registry_digest();
    let routes = crate::materialize_provider_routes(&manifest).expect("provider routes");
    let executable_artifact_digest =
        agent_semantic_content_identity::file_content_digest_v1(std::path::Path::new(
            provider_command_prefix
                .first()
                .expect("provider command prefix"),
        ))
        .expect("digest provider test executable");
    ActivatedProvider {
        manifest_id: manifest.manifest_id,
        manifest_digest,
        language_id: manifest.language_id,
        provider_id: manifest.provider_id,
        binary: manifest.binary,
        execution: manifest.execution,
        execution_command_digest:
            crate::protocol_activation::digest::provider_execution_command_digest(
                &provider_command_prefix,
                &executable_artifact_digest,
            )
            .expect("digest provider execution command"),
        provider_command_prefix,
        namespace: manifest.namespace,
        package_roots: vec!["src".to_string()],
        source_extensions: vec![".ss".to_string()],
        config_files: vec!["gerbil.pkg".to_string()],
        search_capabilities: manifest.search_capabilities,
        language_projection: manifest.language_projection,
        project_resolution: manifest.project_resolution,
        document_resolution: manifest.document_resolution,
        semantic_facts_descriptor: manifest.semantic_facts_descriptor,
        query_pack_descriptor: manifest.query_pack_descriptor,
        semantic_registry_digest,
        policy: manifest.policy,
        routes,
    }
}

fn temp_root(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-runtime-profile-{label}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("temp root");
    root
}

fn write_executable_provider(root: &Path, binary: &str) -> PathBuf {
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("bin dir");
    let path = bin_dir.join(binary);
    write_executable_file(&path);
    path
}

fn write_executable_file(path: &Path) {
    std::fs::create_dir_all(path.parent().expect("executable parent")).expect("bin dir");
    std::fs::write(path, "#!/usr/bin/env sh\nexit 0\n").expect("provider");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("permissions");
    }
}
