use std::path::{Path, PathBuf};

use crate::protocol_activation::digest::provider_manifest_digest;
use crate::protocol_activation::protocol_activation_manifest::{
    ActivatedProvider, ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy,
    HookActivation, HookRuntime,
};
use crate::provider_manifest::provider_manifests;

use super::{
    RuntimeProviderHealthStatus, runtime_profile_invocation, runtime_profiles_for_activation,
    runtime_profiles_for_runtime_with_state_home, runtime_project_root_for_activation,
    runtime_provider_command,
};

#[test]
fn runtime_project_root_for_generated_activation_uses_activation_storage_root() {
    let root = temp_root("activation-project-root");
    std::fs::create_dir_all(root.join(".git")).expect("git marker");
    let state_home = root.join("state-home");
    let resolved = agent_semantic_runtime::state_core::ResolvedState::resolve_with_state_home(
        &root, &state_home,
    )
    .expect("resolve State Home activation path");
    let workspace = resolved
        .ensure_workspace_state_layout()
        .expect("workspace state");
    let canonical_root = std::fs::canonicalize(&root).expect("canonical project root");
    let activation_path = workspace.hook_root().join("state/activation.json");

    assert_eq!(
        runtime_project_root_for_activation(&activation_path, "."),
        canonical_root
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn resolved_provider_binary_materializes_authoritative_command() {
    let root = temp_root("resolved-provider");
    let resolved = write_executable_provider(&root, "asp-rust");
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
fn runtime_profiles_for_activation_accepts_static_provider_identity() {
    let root = temp_root("activation-command-prefix");
    let provider = activated_rust_provider();
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
            search_capabilities: provider.search_capabilities.clone(),
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
    let profiles = runtime_profiles_for_activation(&root, &activation)
        .expect("static activation must not require a Runtime provider binding");
    assert_eq!(profiles.providers.len(), 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn runtime_profiles_for_runtime_resolves_provider_from_explicit_state_home() {
    let root = temp_root("gerbil-command-prefix");
    let state_home = root.join("state-home");
    let state_paths = agent_semantic_runtime::project_state_paths_with_state_home(&root, &state_home)
        .expect("resolve explicit State Home");
    std::fs::create_dir_all(&state_paths.runtime_bin_dir).expect("create runtime bin dir");
    let provider_binary = state_paths.runtime_bin_dir.join("asp-gerbil-scheme");
    write_executable_file(&provider_binary);
    let provider = activated_gerbil_provider();
    let runtime = HookRuntime {
        rankers: Vec::new(),
        project_root: root.display().to_string(),
        providers: vec![provider],
        policy_providers: Vec::new(),
    };
    let provider = &runtime.providers[0];
    let profiles = runtime_profiles_for_runtime_with_state_home(&root, &state_home, &runtime)
        .expect("resolve lazy provider binding");
    let invocation =
        runtime_profile_invocation(&profiles, provider, &["query".into()]).expect("invocation");

    assert_eq!(
        invocation,
        [
            std::fs::canonicalize(&provider_binary)
                .expect("canonical provider binary")
                .display()
                .to_string(),
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
    let state_home = root.join("state-home");
    let state_paths = agent_semantic_runtime::project_state_paths_with_state_home(&root, &state_home)
        .expect("resolve explicit State Home");
    std::fs::create_dir_all(&state_paths.runtime_bin_dir).expect("create runtime bin dir");
    let provider_binary = state_paths.runtime_bin_dir.join("asp-rust");
    write_executable_file(&provider_binary);
    let provider = activated_rust_provider();
    let runtime = HookRuntime {
        rankers: Vec::new(),
        project_root: root.display().to_string(),
        providers: vec![provider],
        policy_providers: Vec::new(),
    };
    std::fs::remove_file(&provider_binary).expect("remove lazy provider executable");

    let profiles = runtime_profiles_for_runtime_with_state_home(&root, &state_home, &runtime)
        .expect("resolve lazy provider health");

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
            .is_some_and(|reason| reason.contains("asp-rust")),
        "{:?}",
        profile.health.reason
    );
    assert!(
        !profile
            .argv
            .iter()
            .any(|arg| arg == &provider_binary.display().to_string())
    );
    let _ = std::fs::remove_dir_all(root);
}

fn activated_rust_provider() -> ActivatedProvider {
    let manifest = provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "rust")
        .expect("rust manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let semantic_registry_digest = crate::semantic_registry_digest();
    let routes = crate::materialize_provider_routes(&manifest).expect("provider routes");
    ActivatedProvider {
        manifest_id: manifest.manifest_id,
        manifest_digest,
        language_id: manifest.language_id,
        provider_id: manifest.provider_id,
        namespace: manifest.namespace,
        package_roots: vec!["src".to_string()],
        source_extensions: vec![".rs".to_string()],
        config_files: vec!["Cargo.toml".to_string()],
        search_capabilities: manifest.search_capabilities,
        project_resolution: manifest.project_resolution,
        document_resolution: manifest.document_resolution,
        semantic_facts_descriptor: manifest.semantic_facts_descriptor,
        query_pack_descriptor: manifest.query_pack_descriptor,
        semantic_registry_digest,
        policy: manifest.policy,
        routes,
    }
}

fn activated_gerbil_provider() -> ActivatedProvider {
    let manifest = provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "gerbil-scheme")
        .expect("gerbil manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let semantic_registry_digest = crate::semantic_registry_digest();
    let routes = crate::materialize_provider_routes(&manifest).expect("provider routes");
    ActivatedProvider {
        manifest_id: manifest.manifest_id,
        manifest_digest,
        language_id: manifest.language_id,
        provider_id: manifest.provider_id,
        namespace: manifest.namespace,
        package_roots: vec!["src".to_string()],
        source_extensions: vec![".ss".to_string()],
        config_files: vec!["gerbil.pkg".to_string()],
        search_capabilities: manifest.search_capabilities,
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
