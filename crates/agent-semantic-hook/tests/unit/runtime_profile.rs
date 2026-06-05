use std::path::{Path, PathBuf};

use crate::protocol_activation::{ActivatedProvider, HookRuntime, provider_manifest_digest};

use super::{
    RUNTIME_PROFILES_PROTOCOL_ID, RUNTIME_PROFILES_PROTOCOL_VERSION, RUNTIME_PROFILES_SCHEMA_ID,
    RUNTIME_PROFILES_SCHEMA_VERSION, RuntimeProfiles, RuntimeProfilesGeneratedBy,
    RuntimeProviderHealth, RuntimeProviderHealthStatus, RuntimeProviderProfile,
    load_or_refresh_runtime_profiles, profiles_match_project_root, profiles_match_runtime,
    runtime_profiles_path_for_activation, write_runtime_profiles_for_runtime,
};

#[test]
fn runtime_profiles_path_for_generated_activation_uses_state_cache_home() {
    let activation_path =
        PathBuf::from("/tmp/project/.cache/agent-semantic-protocol/hooks/activation.json");

    assert_eq!(
        runtime_profiles_path_for_activation(&activation_path),
        PathBuf::from("/tmp/project/.cache/agent-semantic-protocol/runtime/profiles.json")
    );
}

#[test]
fn runtime_profiles_path_for_manual_activation_uses_parent_local_cache() {
    let activation_path = PathBuf::from("/tmp/project/activation.json");

    assert_eq!(
        runtime_profiles_path_for_activation(&activation_path),
        PathBuf::from("/tmp/project/.cache/agent-semantic-protocol/runtime/profiles.json")
    );
}

#[test]
fn runtime_profiles_match_project_root_and_provider_prefix() {
    let provider_prefix = vec!["/repo/.bin/rs-harness".to_string()];
    let runtime = HookRuntime {
        project_root: "/repo".to_string(),
        providers: vec![activated_rust_provider(provider_prefix.clone())],
    };

    assert!(profiles_match_project_root(
        &runtime_profiles("/repo", provider_prefix.clone()),
        Path::new("/repo")
    ));
    assert!(!profiles_match_project_root(
        &runtime_profiles("/tmp/stale-repo", provider_prefix.clone()),
        Path::new("/repo")
    ));
    assert!(profiles_match_runtime(
        &runtime_profiles("/repo", provider_prefix.clone()),
        &runtime
    ));
    assert!(!profiles_match_runtime(
        &runtime_profiles("/repo", Vec::new()),
        &runtime
    ));
}

#[test]
fn load_or_refresh_runtime_profiles_refreshes_project_root_drift() {
    let old_root = temp_root("old");
    let new_root = temp_root("new");
    write_executable_provider(&old_root, "rs-harness");
    let new_provider = write_executable_provider(&new_root, "rs-harness");
    let runtime = HookRuntime {
        project_root: new_root.display().to_string(),
        providers: vec![activated_rust_provider(Vec::new())],
    };
    let profiles_path = new_root.join(".cache/agent-semantic-protocol/runtime/profiles.json");

    write_runtime_profiles_for_runtime(&profiles_path, &old_root, &runtime)
        .expect("write stale profiles");
    let refreshed = load_or_refresh_runtime_profiles(&profiles_path, &new_root, &runtime)
        .expect("refresh stale profiles");

    assert_eq!(refreshed.project_root, new_root.display().to_string());
    assert_eq!(
        refreshed.providers[0].argv,
        [std::fs::canonicalize(&new_provider)
            .expect("canonical new provider")
            .display()
            .to_string()]
    );
    let _ = std::fs::remove_dir_all(old_root);
    let _ = std::fs::remove_dir_all(new_root);
}

fn activated_rust_provider(provider_command_prefix: Vec<String>) -> ActivatedProvider {
    let manifest = crate::provider_manifest::provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "rust")
        .expect("rust manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    ActivatedProvider {
        manifest_id: manifest.manifest_id,
        manifest_digest,
        language_id: manifest.language_id,
        provider_id: manifest.provider_id,
        binary: manifest.binary,
        provider_command_prefix,
        namespace: manifest.namespace,
        package_roots: vec![".".to_string()],
        source_extensions: manifest.source.default_extensions,
        config_files: manifest.source.default_config_files,
        source_roots: manifest.source.default_source_roots,
        ignored_path_prefixes: manifest.source.default_ignored_path_prefixes,
        policy: manifest.policy,
        routes: manifest.routes,
    }
}

fn runtime_profiles(project_root: &str, provider_command_prefix: Vec<String>) -> RuntimeProfiles {
    let manifest = crate::provider_manifest::provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "rust")
        .expect("rust manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let argv = if provider_command_prefix.is_empty() {
        vec![manifest.binary.clone()]
    } else {
        provider_command_prefix.clone()
    };
    RuntimeProfiles {
        schema_id: RUNTIME_PROFILES_SCHEMA_ID.to_string(),
        schema_version: RUNTIME_PROFILES_SCHEMA_VERSION.to_string(),
        protocol_id: RUNTIME_PROFILES_PROTOCOL_ID.to_string(),
        protocol_version: RUNTIME_PROFILES_PROTOCOL_VERSION.to_string(),
        project_root: project_root.to_string(),
        runtime_home: "/repo/.cache/agent-semantic-protocol/runtime".to_string(),
        generated_by: RuntimeProfilesGeneratedBy {
            runtime: "asp".to_string(),
            version: "test".to_string(),
        },
        generated_at: None,
        providers: vec![RuntimeProviderProfile {
            manifest_id: manifest.manifest_id,
            manifest_digest,
            language_id: manifest.language_id,
            provider_id: manifest.provider_id,
            binary: manifest.binary,
            provider_command_prefix,
            resolved_binary: argv.first().cloned(),
            argv,
            health: RuntimeProviderHealth {
                status: RuntimeProviderHealthStatus::Available,
                checked_at: None,
                reason: None,
            },
        }],
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
    std::fs::write(&path, "#!/usr/bin/env sh\nexit 0\n").expect("provider");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("permissions");
    }
    path
}
