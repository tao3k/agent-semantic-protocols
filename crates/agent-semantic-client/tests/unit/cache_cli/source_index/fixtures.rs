use agent_semantic_hook::{
    HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, builtin_provider_manifests, provider_manifest_digest,
};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn write_rust_activation_with_ignored_prefixes(
    root: &Path,
    ignored: &[&str],
) -> std::path::PathBuf {
    let provider_command_prefix = noop_provider_command_prefix();
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "rust")
        .expect("rust manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let semantic_registry_digest = agent_semantic_hook::semantic_registry_digest();
    let execution_command_digest =
        agent_semantic_hook::provider_execution_command_digest(&provider_command_prefix)
            .expect("provider execution command digest");
    let routes =
        agent_semantic_hook::materialize_provider_routes(&manifest).expect("provider routes");
    let activation_project_root =
        std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let activation_path = root.join(".cache/agent-semantic-protocol/hooks/activation.json");
    std::fs::create_dir_all(activation_path.parent().expect("activation parent"))
        .expect("create activation parent");
    let activation = agent_semantic_hook::HookActivation {
        schema_id: HOOK_ACTIVATION_SCHEMA_ID.to_string(),
        schema_version: HOOK_ACTIVATION_SCHEMA_VERSION.to_string(),
        schema_authority: agent_semantic_hook::CANONICAL_SCHEMA_AUTHORITY.to_string(),
        protocol_id: HOOK_PROTOCOL_ID.to_string(),
        protocol_version: HOOK_PROTOCOL_VERSION.to_string(),
        project_root: activation_project_root.display().to_string(),
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
            provider_command_prefix,
            execution_command_digest,
            search_capabilities: manifest.search_capabilities,
            semantic_facts_descriptor: manifest.semantic_facts_descriptor,
            query_pack_descriptor: manifest.query_pack_descriptor,
            semantic_registry_digest,
            routes,
            coverage: agent_semantic_hook::ActivationCoverage {
                package_roots: vec![".".to_string()],
                source_roots: vec!["crates/app/src".to_string(), "vendor/tool/src".to_string()],
                config_files: vec![
                    "Cargo.toml".to_string(),
                    "crates/app/Cargo.toml".to_string(),
                    "vendor/tool/Cargo.toml".to_string(),
                ],
                source_extensions: vec!["rs".to_string()],
                ignored_path_prefixes: ignored.iter().map(|prefix| (*prefix).to_string()).collect(),
            },
        }],
    };
    agent_semantic_hook::write_activation(&activation_path, &activation).expect("write activation");
    activation_path
}

pub(super) fn write_gerbil_activation_with_provider_scope(
    root: &Path,
    provider_bin: &Path,
    source_roots: &[&str],
) -> std::path::PathBuf {
    write_gerbil_activation_with_command_prefix(
        root,
        vec![provider_bin.display().to_string()],
        source_roots,
    )
}

pub(super) fn write_gerbil_activation_with_command_prefix(
    root: &Path,
    provider_command_prefix: Vec<String>,
    source_roots: &[&str],
) -> std::path::PathBuf {
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "gerbil-scheme")
        .expect("gerbil manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let semantic_registry_digest = agent_semantic_hook::semantic_registry_digest();
    let execution_command_digest =
        agent_semantic_hook::provider_execution_command_digest(&provider_command_prefix)
            .expect("provider execution command digest");
    let routes =
        agent_semantic_hook::materialize_provider_routes(&manifest).expect("provider routes");
    let activation_project_root =
        std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let activation_path = root.join(".cache/agent-semantic-protocol/hooks/activation.json");
    std::fs::create_dir_all(activation_path.parent().expect("activation parent"))
        .expect("create activation parent");
    let activation = agent_semantic_hook::HookActivation {
        schema_id: HOOK_ACTIVATION_SCHEMA_ID.to_string(),
        schema_version: HOOK_ACTIVATION_SCHEMA_VERSION.to_string(),
        schema_authority: agent_semantic_hook::CANONICAL_SCHEMA_AUTHORITY.to_string(),
        protocol_id: HOOK_PROTOCOL_ID.to_string(),
        protocol_version: HOOK_PROTOCOL_VERSION.to_string(),
        project_root: activation_project_root.display().to_string(),
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
            provider_command_prefix,
            execution_command_digest,
            search_capabilities: manifest.search_capabilities,
            semantic_facts_descriptor: manifest.semantic_facts_descriptor,
            query_pack_descriptor: manifest.query_pack_descriptor,
            semantic_registry_digest,
            routes,
            coverage: agent_semantic_hook::ActivationCoverage {
                package_roots: vec![".".to_string()],
                source_roots: source_roots
                    .iter()
                    .map(|source_root| (*source_root).to_string())
                    .collect(),
                config_files: vec!["gerbil.pkg".to_string()],
                source_extensions: vec!["ss".to_string()],
                ignored_path_prefixes: Vec::new(),
            },
        }],
    };
    agent_semantic_hook::write_activation(&activation_path, &activation).expect("write activation");
    activation_path
}

pub(super) fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .expect("provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("set executable");
    }
}

pub(super) fn noop_provider_command_prefix() -> Vec<String> {
    ["/usr/bin/true", "/bin/true"]
        .into_iter()
        .map(Path::new)
        .find(|candidate| candidate.is_file())
        .map(|candidate| vec![candidate.display().to_string()])
        .expect("platform true executable")
}

pub(super) fn isolate_home(root: &Path) -> EnvVarGuard {
    let home = root.join("home");
    std::fs::create_dir_all(&home).expect("create isolated home");
    EnvVarGuard::set("HOME", home.as_os_str())
}

pub(super) fn home_local_provider_path(root: &Path, binary: &str) -> std::path::PathBuf {
    root.join("home").join(".local/bin").join(binary)
}

pub(super) struct EnvVarGuard {
    name: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    pub(super) fn set(name: &'static str, value: &OsStr) -> Self {
        let previous = std::env::var_os(name);
        unsafe {
            std::env::set_var(name, value);
        }
        Self { name, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(value) = &self.previous {
                std::env::set_var(self.name, value);
            } else {
                std::env::remove_var(self.name);
            }
        }
    }
}

pub(super) fn run_git(project_root: &Path, args: impl IntoIterator<Item = &'static str>) {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(args)
        .output()
        .expect("run git for source-index fixture");
    assert!(
        output.status.success(),
        "git source-index fixture command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) fn temp_root(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-client-source-index-{label}-{nanos}"));
    std::fs::create_dir_all(root.join(".git")).expect("create temp project root");
    root
}
