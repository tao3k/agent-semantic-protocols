use agent_semantic_hook::{
    HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, RuntimeProviderHealthStatus, builtin_provider_manifests,
    provider_manifest_digest,
};
use serde_json::json;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    ASP_PROVIDER_ACTIVATION_PATH_ENV, LanguageId, ProviderExecution, ProviderId,
    ProviderRegistrySnapshot, ResolvedProvider, RuntimeProfileStatus,
};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn runtime_profile_status_preserves_receipt_labels() {
    assert_eq!(RuntimeProfileStatus::Available.as_str(), "available");
    assert_eq!(RuntimeProfileStatus::Missing.as_str(), "missing");
    assert_eq!(RuntimeProfileStatus::Unexecutable.as_str(), "unexecutable");
}

#[test]
fn runtime_profile_status_maps_from_hook_health_status() {
    assert_eq!(
        RuntimeProfileStatus::from(RuntimeProviderHealthStatus::Available),
        RuntimeProfileStatus::Available
    );
    assert_eq!(
        RuntimeProfileStatus::from(RuntimeProviderHealthStatus::Missing),
        RuntimeProfileStatus::Missing
    );
    assert_eq!(
        RuntimeProfileStatus::from(RuntimeProviderHealthStatus::Unexecutable),
        RuntimeProfileStatus::Unexecutable
    );
}

#[test]
fn activation_provider_prefix_is_metadata_not_client_invocation() {
    let provider = ResolvedProvider {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        binary: "rs-harness".to_string(),
        execution: ProviderExecution::ExternalProcess,
        provider_command_prefix: vec!["./.bin/rs-harness".to_string()],
        runtime_command_argv: Some(vec!["/opt/homebrew/bin/rs-harness".to_string()]),
        runtime_profile_status: Some(RuntimeProfileStatus::Available),
        package_roots: vec![".".to_string()],
        source_roots: vec!["src".to_string()],
        config_files: vec!["Cargo.toml".to_string()],
        source_extensions: vec!["rs".to_string()],
        ignored_path_prefixes: Vec::new(),
    };

    assert_eq!(
        provider.provider_command_prefix,
        vec!["./.bin/rs-harness".to_string()]
    );
    assert_eq!(
        provider.runtime_command_argv,
        Some(vec!["/opt/homebrew/bin/rs-harness".to_string()])
    );
    assert_eq!(
        provider.runtime_profile_status,
        Some(RuntimeProfileStatus::Available)
    );
}

#[test]
fn provider_registry_evidence_tracks_provider_identity_and_existing_scope_dirs() {
    let root = temp_root("provider-registry-evidence");
    std::fs::create_dir_all(root.join("crates/core/src")).expect("create source dir");
    std::fs::write(
        root.join("crates/core/Cargo.toml"),
        "[package]\nname='core'\n",
    )
    .expect("write config file");
    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join(".cache/activation.json"),
        providers: vec![ResolvedProvider {
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            binary: "rs-harness".to_string(),
            execution: ProviderExecution::ExternalProcess,
            provider_command_prefix: vec!["rs-harness".to_string()],
            runtime_command_argv: Some(vec!["/usr/bin/rs-harness".to_string()]),
            runtime_profile_status: Some(RuntimeProfileStatus::Available),
            package_roots: vec!["crates/core".to_string()],
            source_roots: vec!["src".to_string()],
            config_files: vec!["Cargo.toml".to_string()],
            source_extensions: vec!["rs".to_string()],
            ignored_path_prefixes: vec!["target".to_string()],
        }],
    };

    let evidence = snapshot.evidence(&root);

    assert!(evidence.fingerprint.contains("language=rust"));
    assert!(evidence.fingerprint.contains("provider=rs-harness"));
    assert!(evidence.fingerprint.contains("sourceExtensions=rs"));
    assert!(evidence.scope_dirs.contains("."));
    assert!(evidence.scope_dirs.contains("crates/core"));
    assert!(evidence.scope_dirs.contains("crates/core/src"));
    std::fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn activation_snapshot_skips_runtime_profile_when_prefix_is_present() {
    let root = temp_root("activation-prefix-snapshot");
    let activation_path = root.join("activation.json");
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "python")
        .expect("python manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let activation = json!({
        "schemaId": HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": HOOK_ACTIVATION_SCHEMA_VERSION,
        "protocolId": HOOK_PROTOCOL_ID,
        "protocolVersion": HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "generatedBy": { "runtime": "asp", "version": "test" },
        "providers": [{
            "manifestId": manifest.manifest_id,
            "manifestDigest": manifest_digest,
            "languageId": manifest.language_id,
            "providerId": manifest.provider_id,
            "binary": manifest.binary,
            "execution": manifest.execution,
            "providerCommandPrefix": ["missing-python-provider-prefix"],
            "coverage": {
                "packageRoots": ["."],
                "sourceRoots": manifest.source.default_source_roots,
                "configFiles": manifest.source.default_config_files,
                "sourceExtensions": manifest.source.default_extensions,
                "ignoredPathPrefixes": manifest.source.default_ignored_path_prefixes
            }
        }]
    });
    std::fs::write(
        &activation_path,
        serde_json::to_string_pretty(&activation).expect("activation json"),
    )
    .expect("write activation");

    let snapshot = ProviderRegistrySnapshot::load_from_path(&activation_path).expect("snapshot");
    let provider = snapshot
        .provider_for_language(&LanguageId::from("python"))
        .expect("python provider");

    assert_eq!(
        provider.provider_command_prefix,
        vec!["missing-python-provider-prefix".to_string()]
    );
    assert_eq!(provider.runtime_command_argv, None);
    assert_eq!(provider.runtime_profile_status, None);
    std::fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn parent_activation_sync_keeps_requested_project_root() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let _activation_env = EnvVarGuard::unset(ASP_PROVIDER_ACTIVATION_PATH_ENV);
    let _cache_home_env = EnvVarGuard::unset("PRJ_CACHE_HOME");
    let root = temp_root("activation-parent-sync");
    let child = root.join("packages/child");
    let activation_path = root.join(".cache/agent-semantic-protocol/hooks/activation.json");
    std::fs::create_dir_all(&child).expect("create child project");
    std::fs::create_dir_all(activation_path.parent().expect("activation parent"))
        .expect("create activation parent");
    std::fs::write(&activation_path, "{not json").expect("write stale activation");
    let provider_path = write_fake_provider_binary(&root, "py-harness");
    let _path_env = EnvVarGuard::set("PATH", provider_path);

    let snapshot = ProviderRegistrySnapshot::load(&child).expect("snapshot");
    assert_eq!(snapshot.activation_path, activation_path);

    let expected_project_root = child.display().to_string();
    let activation_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&snapshot.activation_path).expect("read activation"),
    )
    .expect("activation json");
    assert_eq!(
        activation_json
            .get("projectRoot")
            .and_then(serde_json::Value::as_str),
        Some(expected_project_root.as_str())
    );
    std::fs::remove_dir_all(root).expect("remove temp root");
}

fn temp_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-client-core-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn write_fake_provider_binary(root: &Path, binary: &str) -> PathBuf {
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create fake provider bin dir");
    let path = bin_dir.join(binary);
    std::fs::write(
        &path,
        "#!/bin/sh\nif [ \"$1\" = \"guide\" ]; then\n  printf '[py-harness-guide]\\n{}\\n'\nfi\nexit 0\n",
    )
    .expect("write fake provider binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&path)
            .expect("fake provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod fake provider");
    }
    bin_dir
}

struct EnvVarGuard {
    name: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn set(name: &'static str, value: impl Into<OsString>) -> Self {
        let previous = std::env::var_os(name);
        let value = value.into();
        unsafe {
            std::env::set_var(name, value);
        }
        Self { name, previous }
    }

    fn unset(name: &'static str) -> Self {
        let previous = std::env::var_os(name);
        unsafe {
            std::env::remove_var(name);
        }
        Self { name, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(value) = self.previous.as_ref() {
                std::env::set_var(self.name, value);
            } else {
                std::env::remove_var(self.name);
            }
        }
    }
}
