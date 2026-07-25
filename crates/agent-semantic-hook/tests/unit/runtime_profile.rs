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
    let resolved = root.join("test-runtime/rs-harness");
    write_executable_file(&resolved);

    let command = runtime_provider_command(Some(&resolved));

    assert_eq!(command.argv, [resolved.display().to_string()]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn runtime_profiles_for_activation_uses_state_home_provider_binary() {
    let _state_home_lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root = temp_root("activation-state-home");
    let state_home = root.join("state-home");
    let runtime_provider =
        write_state_home_provider(&state_home, "rust", "rs-harness", "rs-harness");
    let provider = activated_rust_provider(vec![runtime_provider.display().to_string()]);
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
            provider_command_prefix: Vec::new(),
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
    let previous_state_home = env::var_os(agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV);
    unsafe {
        env::set_var(
            agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV,
            &state_home,
        );
    }

    let profiles = runtime_profiles_for_activation(&root, &activation).expect("profiles");
    let invocation =
        runtime_profile_invocation(&profiles, &provider, &["query".into()]).expect("invocation");

    match previous_state_home {
        Some(value) => unsafe {
            env::set_var(
                agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV,
                value,
            );
        },
        None => unsafe {
            env::remove_var(agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV);
        },
    }
    assert_eq!(
        invocation,
        [
            std::fs::canonicalize(&runtime_provider)
                .expect("canonical runtime provider")
                .display()
                .to_string(),
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
fn runtime_profiles_for_runtime_uses_state_home_provider_binary() {
    let _state_home_lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root = temp_root("gerbil-state-home");
    let state_home = root.join("state-home");
    let runtime_gslph = write_state_home_provider(
        &state_home,
        "gerbil-scheme",
        "gerbil-scheme-harness",
        "gslph",
    );
    let provider = activated_gerbil_provider(vec![runtime_gslph.display().to_string()]);
    let runtime = HookRuntime {
        project_root: root.display().to_string(),
        providers: vec![provider],
    };
    let provider = &runtime.providers[0];
    let previous_state_home = env::var_os(agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV);
    unsafe {
        env::set_var(
            agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV,
            &state_home,
        );
    }

    let profiles = runtime_profiles_for_runtime(&root, &runtime);
    let invocation =
        runtime_profile_invocation(&profiles, provider, &["query".into()]).expect("invocation");

    match previous_state_home {
        Some(value) => unsafe {
            env::set_var(
                agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV,
                value,
            );
        },
        None => unsafe {
            env::remove_var(agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV);
        },
    }
    assert_eq!(
        invocation,
        [
            std::fs::canonicalize(&runtime_gslph)
                .expect("canonical runtime gslph")
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
fn runtime_profiles_for_runtime_fails_closed_when_state_home_binary_is_missing() {
    let _state_home_lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root = temp_root("missing-state-home-provider");
    let state_home = root.join("state-home");
    let runtime_provider =
        write_state_home_provider(&state_home, "rust", "rs-harness", "rs-harness");
    let provider = activated_rust_provider(vec![runtime_provider.display().to_string()]);
    let runtime = HookRuntime {
        project_root: root.display().to_string(),
        providers: vec![provider],
    };
    std::fs::remove_file(&runtime_provider).expect("remove State Home provider after activation");
    std::fs::remove_file(state_home.join("runtime/provider-locks/rust.lock.toml"))
        .expect("remove State Home provider receipt after activation");
    let previous_state_home = env::var_os(agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV);
    unsafe {
        env::set_var(
            agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV,
            &state_home,
        );
    }

    let profiles = runtime_profiles_for_runtime(&root, &runtime);

    match previous_state_home {
        Some(value) => unsafe {
            env::set_var(
                agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV,
                value,
            );
        },
        None => unsafe {
            env::remove_var(agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV);
        },
    }
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
            .is_some_and(|reason| reason.contains("state-home/runtime/bin/rs-harness")),
        "{:?}",
        profile.health.reason
    );
    assert!(profile.argv.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn runtime_profiles_for_runtime_fails_closed_when_state_home_receipt_drifts() {
    let _state_home_lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root = temp_root("drifted-state-home-provider");
    let state_home = root.join("state-home");
    let runtime_provider =
        write_state_home_provider(&state_home, "rust", "rs-harness", "rs-harness");
    let provider = activated_rust_provider(vec![runtime_provider.display().to_string()]);
    let runtime = HookRuntime {
        project_root: root.display().to_string(),
        providers: vec![provider],
    };
    std::fs::write(&runtime_provider, "#!/usr/bin/env sh\nexit 7\n")
        .expect("replace State Home provider after lock receipt");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&runtime_provider)
            .expect("drifted provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&runtime_provider, permissions)
            .expect("preserve drifted provider executability");
    }
    let previous_state_home = env::var_os(agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV);
    unsafe {
        env::set_var(
            agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV,
            &state_home,
        );
    }

    let profiles = runtime_profiles_for_runtime(&root, &runtime);

    match previous_state_home {
        Some(value) => unsafe {
            env::set_var(
                agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV,
                value,
            );
        },
        None => unsafe {
            env::remove_var(agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV);
        },
    }
    let profile = profiles
        .providers
        .first()
        .expect("runtime provider profile");
    assert_eq!(
        profile.health.status,
        RuntimeProviderHealthStatus::Unexecutable
    );
    assert!(profile.resolved_binary.is_none());
    assert!(profile.argv.is_empty());
    assert!(
        profile
            .health
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("digest") || reason.contains("metadata")),
        "{:?}",
        profile.health.reason
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
        provider_command_prefix: Vec::new(),
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
        provider_command_prefix: Vec::new(),
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

fn write_state_home_provider(
    state_home: &Path,
    language_id: &str,
    provider_id: &str,
    binary: &str,
) -> PathBuf {
    let path = state_home.join("runtime/bin").join(binary);
    write_executable_file(&path);
    let entrypoint_digest = agent_semantic_content_identity::file_content_digest_v1(&path)
        .expect("provider content digest");
    let metadata_digest = agent_semantic_content_identity::file_artifact_metadata_digest_v1(&path)
        .expect("provider metadata digest");
    let lock_dir = state_home.join("runtime/provider-locks");
    std::fs::create_dir_all(&lock_dir).expect("provider lock dir");
    std::fs::write(
        lock_dir.join(format!("{language_id}.lock.toml")),
        format!(
            "schemaId = \"asp.provider-install-lock.v1\"\nprovider = \"{provider_id}\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{entrypoint_digest}\"\ninstalledEntrypointMetadataDigest = \"{metadata_digest}\"\n",
            path.display()
        ),
    )
    .expect("provider install lock");
    std::fs::canonicalize(&path).expect("canonical State Home provider")
}
use crate::runtime_profile::runtime_provider_command;
