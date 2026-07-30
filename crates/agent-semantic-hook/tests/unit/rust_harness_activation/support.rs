use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_hook::{
    HookRuntime, builtin_provider_manifests, default_client_config_template, parse_hook_activation,
    provider_manifest_digest,
};

pub(super) fn asp_command() -> Command {
    Command::new(asp_binary_path())
}

fn target_debug_asp() -> Option<PathBuf> {
    let current_exe = std::env::current_exe().ok()?;
    let debug_dir = current_exe.parent()?.parent()?;
    let asp = debug_dir.join(format!("asp{}", std::env::consts::EXE_SUFFIX));
    if !asp.exists() {
        return None;
    }
    assert_asp_binary_fresh(&asp);
    Some(asp)
}

pub(super) fn asp_bin_dir() -> PathBuf {
    asp_binary_path()
        .parent()
        .expect("asp binary must have parent directory")
        .to_path_buf()
}

fn asp_binary_path() -> PathBuf {
    if let Ok(path) = std::env::var("ASP_TEST_ASP_BIN") {
        return checked_asp_path(PathBuf::from(path), "ASP_TEST_ASP_BIN");
    }
    if let Some(path) = option_env!("CARGO_BIN_EXE_asp") {
        return checked_asp_path(PathBuf::from(path), "CARGO_BIN_EXE_asp");
    }
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_asp") {
        return checked_asp_path(PathBuf::from(path), "CARGO_BIN_EXE_asp");
    }
    if let Some(path) = target_debug_asp() {
        return path;
    }
    panic!(
        "agent-semantic-hook CLI tests require a fresh asp binary; run `cargo build -p agent-semantic-protocol --bin asp` or set ASP_TEST_ASP_BIN"
    );
}

fn checked_asp_path(path: PathBuf, source: &str) -> PathBuf {
    assert!(
        path.exists(),
        "{source} points to a missing asp binary: {}",
        path.display()
    );
    assert_asp_binary_fresh(&path);
    path
}

fn assert_asp_binary_fresh(binary: &Path) {
    let Some(newest_source) = newest_asp_hook_surface_source_mtime() else {
        return;
    };
    let binary_mtime = binary
        .metadata()
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    assert!(
        binary_mtime >= newest_source,
        "asp binary {} is older than hook/install sources; rebuild with `cargo build -p agent-semantic-protocol --bin asp`",
        binary.display()
    );
}

fn newest_asp_hook_surface_source_mtime() -> Option<SystemTime> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .parent()?
        .to_path_buf();
    [
        "crates/agent-semantic-protocol/src/main.rs",
        "crates/agent-semantic-protocol/src/command/dispatch.rs",
        "crates/agent-semantic-protocol/src/command/hook.rs",
        "crates/agent-semantic-protocol/src/command/hook_runtime.rs",
        "crates/agent-semantic-protocol/src/command/hook_runtime_codex_plugin.rs",
        "crates/agent-semantic-protocol/src/command/hook_runtime_install.rs",
        "crates/agent-semantic-protocol/src/command/hook_runtime_subagent.rs",
        "crates/agent-semantic-protocol/src/command/install_provider.rs",
        "crates/agent-semantic-protocol/src/command/org_archive.rs",
        "crates/agent-semantic-protocol/src/command/org_capture.rs",
        "crates/agent-semantic-protocol/src/command/org_capture_contract_materialize.rs",
        "crates/agent-semantic-protocol/src/command/hook_enforcement.rs",
        "crates/agent-semantic-config/src/hook_client_config.rs",
        "crates/agent-semantic-hook/src/activation_store.rs",
        "crates/agent-semantic-hook/src/event_state.rs",
        "crates/agent-semantic-hook/src/executable.rs",
        "crates/agent-semantic-hook/src/hook_config/agent_org_config.rs",
        "crates/agent-semantic-hook/src/provider_manifest.rs",
        "crates/agent-semantic-hook/src/provider_registry.rs",
        "crates/agent-semantic-hook/src/runtime_profile.rs",
        "crates/agent-semantic-hook/src/protocol_activation/digest.rs",
        "crates/agent-semantic-hook/src/protocol_activation/protocol_activation_manifest.rs",
        "crates/agent-semantic-hook/src/protocol_activation/protocol_activation_runtime.rs",
        "languages/rust-lang-project-harness/provider/asp-provider-manifest.json",
        "schemas/semantic-language-registry.providers.v1.json",
        "SKILL.org",
        "SKILL.contract.org",
    ]
    .into_iter()
    .filter_map(|relative| root.join(relative).metadata().ok()?.modified().ok())
    .max()
}

pub(super) fn temp_project_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-hook-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp project root");
    std::fs::create_dir_all(root.join(".git")).expect("create temp git marker");
    root
}

pub(super) fn root_owned_rust_activation_json() -> String {
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id().as_str() == "rust")
        .expect("rust manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("digest manifest");
    let routes = agent_semantic_hook::materialize_provider_routes(&manifest).expect("rust routes");
    let state_home = std::env::temp_dir().join(format!(
        "agent-semantic-hook-root-owned-state-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let provider_executable =
        write_state_home_provider_binary(&state_home, "rust", "rs-harness", "rs-harness");
    let resolved_execution_prefix = vec![provider_executable.display().to_string()];
    let provider_artifact = agent_semantic_hook::active_provider_artifact_input_with_state_home(
        std::path::Path::new("."),
        &state_home,
        manifest.language_id(),
        manifest.provider_id(),
        provider_executable,
    )
    .expect("validate provider test executable against its install receipt");
    let execution_command_digest = agent_semantic_hook::provider_execution_command_digest(
        &resolved_execution_prefix,
        &provider_artifact.artifact_digest,
    )
    .expect("digest provider execution command");
    let activation = agent_semantic_hook::HookActivation {
        rankers: Vec::new(),
        schema_id: agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_ID.to_string(),
        schema_version: agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_VERSION.to_string(),
        schema_authority: "https://tao3k.github.io/agent-semantic-protocols/schemas/".to_string(),
        protocol_id: agent_semantic_hook::HOOK_PROTOCOL_ID.to_string(),
        protocol_version: agent_semantic_hook::HOOK_PROTOCOL_VERSION.to_string(),
        project_root: ".".to_string(),
        generated_by: agent_semantic_hook::ActivationGeneratedBy {
            runtime: "asp".to_string(),
            version: "test".to_string(),
        },
        generated_at: None,
        providers: vec![agent_semantic_hook::ActivatedProviderConfig {
            manifest_id: manifest.manifest_id().to_string(),
            manifest_digest,
            language_id: manifest.language_id().clone(),
            provider_id: manifest.provider_id().clone(),
            binary: manifest.binary().to_string(),
            execution: manifest.execution(),
            provider_command_prefix: Vec::new(),
            execution_command_digest,
            search_capabilities: manifest.search_capabilities().clone(),
            semantic_facts_descriptor: manifest.semantic_facts_descriptor().cloned(),
            query_pack_descriptor: manifest.query_pack_descriptor().clone(),
            semantic_registry_digest: agent_semantic_hook::semantic_registry_digest(),
            routes,
            coverage: agent_semantic_hook::ActivationCoverage {
                package_roots: vec![".".to_string()],
                config_files: manifest.source().default_config_files.clone(),
                source_extensions: manifest.source().default_extensions.clone(),
            },
        }],
    };
    serde_json::to_string_pretty(&activation).expect("serialize root-owned rust activation")
}

pub(super) fn write_root_owned_rust_activation(root: &std::path::Path) -> PathBuf {
    let path = root.join("rust-activation.json");
    std::fs::write(&path, root_owned_rust_activation_json()).expect("write rust activation");
    path
}

pub(super) fn write_default_client_hook_config(root: &std::path::Path) -> PathBuf {
    let path = root
        .join(".agent-semantic-protocols")
        .join("hooks")
        .join("config.toml");
    std::fs::create_dir_all(path.parent().expect("client hook config parent"))
        .expect("create client hook config parent");
    std::fs::write(&path, default_client_config_template()).expect("write client hook config");
    path
}

pub(super) fn write_state_home_provider_binary(
    state_home: &std::path::Path,
    language_id: &str,
    provider_id: &str,
    binary: &str,
) -> PathBuf {
    write_state_home_provider_file(state_home, language_id, provider_id, binary, 0o755, false)
}

pub(super) fn write_failing_state_home_provider_binary(
    state_home: &std::path::Path,
    language_id: &str,
    provider_id: &str,
    binary: &str,
) -> PathBuf {
    write_state_home_provider_file(state_home, language_id, provider_id, binary, 0o755, true)
}

fn write_state_home_provider_file(
    state_home: &std::path::Path,
    language_id: &str,
    provider_id: &str,
    binary: &str,
    mode: u32,
    fail_on_execute: bool,
) -> PathBuf {
    let bin_dir = state_home.join("runtime").join("bin");
    std::fs::create_dir_all(&bin_dir).expect("create State Home runtime bin");
    let path = bin_dir.join(binary);
    let script = if fail_on_execute {
        "#!/bin/sh\nprintf 'provider process should not be executed\\n' >&2\nexit 42\n".to_string()
    } else {
        let guide_marker = match binary {
            "rs-harness" => {
                "[agent-guide] runtime=agent-semantic-hook language=rust provider=rs-harness"
            }
            "ts-harness" => "[ts-harness-guide]",
            "py-harness" | "custom-py-harness" => "[py-harness-guide]",
            _ => "[agent-guide]",
        };
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"guide\" ]; then\n  printf '%s\\n' '{}'\n  exit 0\nfi\nexit 0\n",
            guide_marker
        )
    };
    std::fs::write(&path, script).expect("write State Home provider binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&path)
            .expect("State Home provider metadata")
            .permissions();
        permissions.set_mode(mode);
        std::fs::set_permissions(&path, permissions).expect("chmod State Home provider");
    }
    let entrypoint_digest = agent_semantic_content_identity::file_content_digest_v1(&path)
        .expect("provider content digest");
    let metadata_digest = agent_semantic_content_identity::file_artifact_metadata_digest_v1(&path)
        .expect("provider metadata digest");
    let lock_dir = agent_semantic_runtime::provider_receipt_dir(&state_home);
    std::fs::create_dir_all(&lock_dir).expect("create provider lock dir");
    std::fs::write(
        lock_dir.join(format!("{language_id}.lock.toml")),
        format!(
            "schemaId = \"asp.provider-install-lock.v1\"\nprovider = \"{provider_id}\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{entrypoint_digest}\"\ninstalledEntrypointMetadataDigest = \"{metadata_digest}\"\n",
            path.display()
        ),
    )
    .expect("write provider install receipt");
    std::fs::canonicalize(&path).unwrap_or(path)
}

pub(super) fn write_unmanaged_provider_file(
    root: &std::path::Path,
    binary: &str,
    mode: u32,
) -> PathBuf {
    write_unmanaged_provider_in_dir(&root.join(".bin"), binary, mode)
}

fn write_unmanaged_provider_in_dir(bin_dir: &std::path::Path, binary: &str, mode: u32) -> PathBuf {
    std::fs::create_dir_all(bin_dir).expect("create fake provider bin dir");
    let path = bin_dir.join(binary);
    let guide_marker = match binary {
        "rs-harness" => {
            "[agent-guide] runtime=agent-semantic-hook language=rust provider=rs-harness"
        }
        "ts-harness" => "[ts-harness-guide]",
        "py-harness" => "[py-harness-guide]",
        _ => "[agent-guide]",
    };
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"guide\" ]; then\n  printf '%s\\n' '{}'\n  exit 0\nfi\nexit 0\n",
            guide_marker
        ),
    )
    .expect("write fake provider binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(&path)
            .expect("fake provider metadata")
            .permissions();
        permissions.set_mode(mode);
        std::fs::set_permissions(&path, permissions).expect("chmod fake provider");
    }
    bin_dir.to_path_buf()
}

pub(super) fn rust_harness_activation() -> HookRuntime {
    parse_hook_activation(&root_owned_rust_activation_json())
        .expect("valid root-owned rust activation")
}
