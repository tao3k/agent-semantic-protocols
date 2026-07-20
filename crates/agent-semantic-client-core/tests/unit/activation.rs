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
    ASP_PROVIDER_ACTIVATION_PATH_ENV, LanguageId, ProviderId, ProviderRegistrySnapshot,
    ResolvedProvider, RuntimeProfileStatus, test_support::resolved_provider,
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
    let mut provider = resolved_provider();
    provider.provider_command_prefix = vec!["./.bin/rs-harness".to_string()];
    provider.runtime_command_argv = Some(vec!["/opt/homebrew/bin/rs-harness".to_string()]);
    provider.runtime_profile_status = Some(RuntimeProfileStatus::Available);

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
    let mut provider = resolved_provider();
    provider.package_roots = vec!["crates/core".to_string()];
    provider.source_roots = vec!["src".to_string()];
    provider.config_files = vec!["Cargo.toml".to_string()];
    provider.source_extensions = vec!["rs".to_string()];
    provider.ignored_path_prefixes = vec!["target".to_string()];
    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join(".cache/activation.json"),
        providers: vec![provider],
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
fn provider_registry_fingerprint_binds_complete_semantic_descriptor() {
    let root = temp_root("provider-registry-semantic-fingerprint");
    std::fs::create_dir_all(&root).expect("create root");
    let baseline_provider = resolved_provider();
    let baseline = provider_evidence_fingerprint(&root, baseline_provider.clone());
    let baseline_schemas = baseline_provider
        .semantic_facts_descriptor
        .as_ref()
        .expect("semantic descriptor")
        .packet_schema_ids
        .clone();

    let mut mutations = Vec::new();
    let mut descriptor_id = baseline_provider.clone();
    descriptor_id
        .semantic_facts_descriptor
        .as_mut()
        .expect("semantic descriptor")
        .descriptor_id = "semantic.changed".to_string();
    mutations.push(descriptor_id);
    let mut descriptor_version = baseline_provider.clone();
    descriptor_version
        .semantic_facts_descriptor
        .as_mut()
        .expect("semantic descriptor")
        .descriptor_version = "2".to_string();
    mutations.push(descriptor_version);
    let mut language = baseline_provider.clone();
    language.language_id = LanguageId::from("rust-variant");
    mutations.push(language);
    let mut producer = baseline_provider.clone();
    producer.provider_id = ProviderId::from("rs-harness-variant");
    mutations.push(producer);
    let mut families = baseline_provider.clone();
    families
        .semantic_facts_descriptor
        .as_mut()
        .expect("semantic descriptor")
        .fact_kinds = vec!["changed-family".to_string()];
    mutations.push(families);
    let mut intent = baseline_provider;
    intent
        .semantic_facts_descriptor
        .as_mut()
        .expect("semantic descriptor")
        .intent_axes[0]
        .terms = vec!["changed-intent".to_string()];
    mutations.push(intent);
    let mut absent = resolved_provider();
    absent.semantic_facts_descriptor = None;
    mutations.push(absent);

    for mutated in mutations {
        if let Some(descriptor) = mutated.semantic_facts_descriptor.as_ref() {
            assert_eq!(descriptor.packet_schema_ids, baseline_schemas);
        }
        assert_ne!(provider_evidence_fingerprint(&root, mutated), baseline);
    }
    std::fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn provider_registry_fingerprint_binds_complete_search_capabilities() {
    let root = temp_root("provider-registry-search-capabilities-fingerprint");
    std::fs::create_dir_all(&root).expect("create root");
    let baseline_provider = resolved_provider();
    let baseline = provider_evidence_fingerprint(&root, baseline_provider.clone());
    let mut mutations = Vec::new();

    let mut owner_items = baseline_provider.clone();
    owner_items.search_capabilities.owner_items = !owner_items.search_capabilities.owner_items;
    mutations.push(owner_items);
    let mut semantic_facts = baseline_provider.clone();
    semantic_facts.search_capabilities.semantic_facts =
        !semantic_facts.search_capabilities.semantic_facts;
    mutations.push(semantic_facts);
    let mut dependency_topology = baseline_provider.clone();
    dependency_topology.search_capabilities.dependency_topology =
        !dependency_topology.search_capabilities.dependency_topology;
    mutations.push(dependency_topology);
    let mut dependency_topology_metadata = baseline_provider.clone();
    dependency_topology_metadata
        .search_capabilities
        .dependency_topology_metadata = !dependency_topology_metadata
        .search_capabilities
        .dependency_topology_metadata;
    mutations.push(dependency_topology_metadata);
    let mut workspace_scope = baseline_provider.clone();
    workspace_scope.search_capabilities.workspace_scope =
        !workspace_scope.search_capabilities.workspace_scope;
    mutations.push(workspace_scope);
    let mut source_snapshot = baseline_provider;
    source_snapshot.search_capabilities.source_snapshot = None;
    mutations.push(source_snapshot);

    for mutated in mutations {
        assert_ne!(provider_evidence_fingerprint(&root, mutated), baseline);
    }
    std::fs::remove_dir_all(root).expect("remove temp root");
}

fn provider_evidence_fingerprint(root: &std::path::Path, provider: ResolvedProvider) -> String {
    ProviderRegistrySnapshot {
        activation_path: root.join(".cache/activation.json"),
        providers: vec![provider],
    }
    .evidence(root)
    .fingerprint
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
    let routes = agent_semantic_hook::materialize_provider_routes(&manifest)
        .expect("python provider routes");
    let activation = agent_semantic_hook::HookActivation {
        schema_id: HOOK_ACTIVATION_SCHEMA_ID.to_string(),
        schema_version: HOOK_ACTIVATION_SCHEMA_VERSION.to_string(),
        protocol_id: HOOK_PROTOCOL_ID.to_string(),
        protocol_version: HOOK_PROTOCOL_VERSION.to_string(),
        project_root: ".".to_string(),
        generated_by: agent_semantic_hook::ActivationGeneratedBy {
            runtime: "asp".to_string(),
            version: "test".to_string(),
        },
        generated_at: None,
        providers: vec![agent_semantic_hook::ActivatedProviderConfig {
            manifest_id: manifest.manifest_id,
            manifest_digest,
            language_id: manifest.language_id,
            provider_id: manifest.provider_id,
            binary: manifest.binary,
            execution: manifest.execution,
            provider_command_prefix: vec!["missing-python-provider-prefix".to_string()],
            search_capabilities: manifest.search_capabilities,
            semantic_facts_descriptor: manifest.semantic_facts_descriptor,
            query_pack_descriptor: manifest.query_pack_descriptor,
            semantic_registry_digest: agent_semantic_hook::semantic_registry_digest(),
            routes,
            coverage: agent_semantic_hook::ActivationCoverage {
                package_roots: vec![".".to_string()],
                source_roots: manifest.source.default_source_roots,
                config_files: manifest.source.default_config_files,
                source_extensions: manifest.source.default_extensions,
                ignored_path_prefixes: manifest.source.default_ignored_path_prefixes,
            },
        }],
    };
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
fn explicit_activation_path_keeps_requested_project_root() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let _cache_home_env = EnvVarGuard::unset("PRJ_CACHE_HOME");
    let root = temp_root("activation-parent-sync");
    let child = root.join("packages/child");
    let activation_path = root.join("state/hooks/activation.json");
    let _activation_env = EnvVarGuard::set(ASP_PROVIDER_ACTIVATION_PATH_ENV, &activation_path);
    std::fs::create_dir_all(&child).expect("create child project");
    std::fs::create_dir_all(activation_path.parent().expect("activation parent"))
        .expect("create activation parent");
    let provider_path = write_fake_provider_binary(&root, "py-harness");
    let _path_env = EnvVarGuard::set("PATH", provider_path);
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "python")
        .expect("python manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let expected_project_root = child.display().to_string();
    let routes = agent_semantic_hook::materialize_provider_routes(&manifest)
        .expect("python provider routes");
    let activation = agent_semantic_hook::HookActivation {
        schema_id: HOOK_ACTIVATION_SCHEMA_ID.to_string(),
        schema_version: HOOK_ACTIVATION_SCHEMA_VERSION.to_string(),
        protocol_id: HOOK_PROTOCOL_ID.to_string(),
        protocol_version: HOOK_PROTOCOL_VERSION.to_string(),
        project_root: expected_project_root.clone(),
        generated_by: agent_semantic_hook::ActivationGeneratedBy {
            runtime: "asp".to_string(),
            version: "test".to_string(),
        },
        generated_at: None,
        providers: vec![agent_semantic_hook::ActivatedProviderConfig {
            manifest_id: manifest.manifest_id,
            manifest_digest,
            language_id: manifest.language_id,
            provider_id: manifest.provider_id,
            binary: manifest.binary,
            execution: manifest.execution,
            provider_command_prefix: vec!["py-harness".to_string()],
            search_capabilities: manifest.search_capabilities,
            semantic_facts_descriptor: manifest.semantic_facts_descriptor,
            query_pack_descriptor: manifest.query_pack_descriptor,
            semantic_registry_digest: agent_semantic_hook::semantic_registry_digest(),
            routes,
            coverage: agent_semantic_hook::ActivationCoverage {
                package_roots: vec![".".to_string()],
                source_roots: manifest.source.default_source_roots,
                config_files: manifest.source.default_config_files,
                source_extensions: manifest.source.default_extensions,
                ignored_path_prefixes: manifest.source.default_ignored_path_prefixes,
            },
        }],
    };
    std::fs::write(
        &activation_path,
        serde_json::to_string_pretty(&activation).expect("activation json"),
    )
    .expect("write activation");

    let snapshot = ProviderRegistrySnapshot::load(&child).expect("snapshot");
    assert_eq!(snapshot.activation_path, activation_path);

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
