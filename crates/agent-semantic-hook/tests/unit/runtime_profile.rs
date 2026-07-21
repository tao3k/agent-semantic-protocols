use std::env;
use std::path::{Path, PathBuf};

use crate::protocol_activation::digest::provider_manifest_digest;
use crate::protocol_activation::protocol_activation_manifest::{
    ActivatedProvider, ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy,
    HookActivation, HookRuntime,
};
use crate::provider_manifest::provider_manifests;

use super::{
    RuntimeProviderHealthStatus, runtime_profile_invocation, runtime_profiles_for_activation,
    runtime_profiles_for_runtime, runtime_project_root_for_activation,
};

#[test]
fn runtime_project_root_for_generated_activation_uses_activation_storage_root() {
    let activation_path =
        PathBuf::from("/tmp/project/.cache/agent-semantic-protocol/hooks/activation.json");

    assert_eq!(
        runtime_project_root_for_activation(&activation_path, "."),
        PathBuf::from("/tmp/project")
    );
}

#[test]
fn runtime_profiles_for_activation_uses_provider_command_prefix() {
    let root = temp_root("activation-prefix");
    let wrapper = write_executable_provider(&root, "provider-wrapper");
    let provider = activated_rust_provider(vec![
        wrapper.display().to_string(),
        "rs-harness".to_string(),
    ]);
    let activation = HookActivation {
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
            semantic_facts_descriptor: provider.semantic_facts_descriptor.clone(),
            query_pack_descriptor: provider.query_pack_descriptor.clone(),
            semantic_registry_digest: provider.semantic_registry_digest.clone(),
            routes: provider.routes.clone(),
            coverage: ActivationCoverage {
                package_roots: provider.package_roots.clone(),
                source_roots: provider.source_roots.clone(),
                config_files: provider.config_files.clone(),
                source_extensions: provider.source_extensions.clone(),
                ignored_path_prefixes: provider.ignored_path_prefixes.clone(),
            },
        }],
    };

    let profiles = runtime_profiles_for_activation(&root, &activation).expect("profiles");
    let invocation =
        runtime_profile_invocation(&profiles, &provider, &["query".into()]).expect("invocation");

    assert_eq!(
        invocation,
        [
            std::fs::canonicalize(&wrapper)
                .expect("canonical wrapper")
                .display()
                .to_string(),
            "rs-harness".to_string(),
            "query".to_string(),
        ]
    );
    assert!(
        !root
            .join(".cache/agent-semantic-protocol/runtime/profiles.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn runtime_profiles_for_runtime_prefers_home_local_gslph_for_gerbil() {
    let root = temp_root("gerbil-home-local");
    let home = root.join("home");
    let home_gslph = home.join(".local/bin/gslph");
    write_executable_file(&home_gslph);
    let _project_gslph = write_executable_provider(&root, "gslph");
    let wrapper = write_executable_provider(&root, "asp");
    let provider = activated_gerbil_provider(vec![
        wrapper.display().to_string(),
        "gerbil-scheme".to_string(),
    ]);
    let runtime = HookRuntime {
        project_root: root.display().to_string(),
        providers: vec![provider],
    };
    let provider = &runtime.providers[0];
    let previous_home = env::var_os("HOME");
    unsafe {
        env::set_var("HOME", &home);
    }

    let profiles = runtime_profiles_for_runtime(&root, &runtime);
    let invocation =
        runtime_profile_invocation(&profiles, provider, &["query".into()]).expect("invocation");

    match previous_home {
        Some(value) => unsafe {
            env::set_var("HOME", value);
        },
        None => unsafe {
            env::remove_var("HOME");
        },
    }
    assert_eq!(
        invocation,
        [
            std::fs::canonicalize(&home_gslph)
                .expect("canonical home gslph")
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

fn activated_rust_provider(provider_command_prefix: Vec<String>) -> ActivatedProvider {
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
        binary: manifest.binary,
        execution: manifest.execution,
        execution_command_digest:
            crate::protocol_activation::digest::provider_execution_command_digest(
                &provider_command_prefix,
            )
            .expect("digest provider execution command"),
        provider_command_prefix,
        namespace: manifest.namespace,
        package_roots: vec![".".to_string()],
        source_extensions: manifest.source.default_extensions,
        config_files: manifest.source.default_config_files,
        source_roots: manifest.source.default_source_roots,
        ignored_path_prefixes: manifest.source.default_ignored_path_prefixes,
        search_capabilities: manifest.search_capabilities,
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
            )
            .expect("digest provider execution command"),
        provider_command_prefix,
        namespace: manifest.namespace,
        package_roots: vec![".".to_string()],
        source_extensions: manifest.source.default_extensions,
        config_files: manifest.source.default_config_files,
        source_roots: manifest.source.default_source_roots,
        ignored_path_prefixes: manifest.source.default_ignored_path_prefixes,
        search_capabilities: manifest.search_capabilities,
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
