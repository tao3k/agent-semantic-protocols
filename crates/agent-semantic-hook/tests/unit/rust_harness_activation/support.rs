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
    assert_asp_binary_compatible(&asp);
    Some(asp)
}

pub(super) fn asp_bin_dir(root: &Path) -> PathBuf {
    let bin_dir = root.join(".asp-test-bin");
    std::fs::create_dir_all(&bin_dir).expect("create isolated ASP bin dir");
    std::fs::copy(
        asp_binary_path(),
        bin_dir.join(format!("asp{}", std::env::consts::EXE_SUFFIX)),
    )
    .expect("copy isolated ASP bin");
    bin_dir
}

pub(super) fn asp_binary_path() -> PathBuf {
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
    assert_asp_binary_compatible(&path);
    path
}

fn assert_asp_binary_compatible(binary: &Path) {
    let output = Command::new(binary)
        .arg("--contract-fingerprint")
        .output()
        .unwrap_or_else(|error| panic!("execute ASP contract fingerprint probe: {error}"));
    let actual = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let expected = agent_semantic_config::hook_client_contract_fingerprint();
    assert!(
        output.status.success() && actual == expected,
        "asp binary {} has incompatible Hook contract: expected={expected} actual={actual} stderr={}",
        binary.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) fn temp_project_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-hook-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp project root");
    let _git_fixture = crate::integration_fixture::GIT_FIXTURE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let output = crate::integration_fixture::isolated_git_command()
        .args(["init", "--quiet", "--template="])
        .current_dir(&root)
        .output()
        .expect("initialize temporary Git workspace");
    assert!(
        output.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let agents_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("agents");
    let fixture_agents = root.join("agents");
    std::fs::create_dir_all(&fixture_agents).expect("create fixture agent registry");
    for name in [
        "config.toml",
        "asp_explorer_codex.toml",
        "asp_explorer_claude.md",
        "asp_testing_codex.toml",
        "asp_testing_claude.md",
    ] {
        std::fs::copy(agents_root.join(name), fixture_agents.join(name))
            .expect("copy fixture agent registry entry");
    }
    root
}

pub(super) fn stage_project_candidates(root: &Path) {
    let _git_fixture = crate::integration_fixture::GIT_FIXTURE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let output = crate::integration_fixture::isolated_git_command()
        .args(["add", "."])
        .current_dir(root)
        .output()
        .expect("stage temporary project candidates");
    assert!(
        output.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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
                config_files: manifest
                    .project_resolution()
                    .expect("Rust project scope")
                    .entry_markers
                    .clone(),
                source_extensions: vec![".rs".to_string()],
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
        let (project_entry, source_extension) = match language_id {
            "rust" => ("Cargo.toml", ".rs"),
            "typescript" => ("package.json", ".ts"),
            "python" => ("pyproject.toml", ".py"),
            "julia" => ("Project.toml", ".jl"),
            "gerbil-scheme" => ("gerbil.pkg", ".ss"),
            other => panic!("test provider omitted project fixture mapping: {other}"),
        };
        let project_resolution = serde_json::json!({
            "schemaId": "agent.semantic-protocols.provider-project-resolution-response",
            "schemaVersion": "1",
            "languageId": language_id,
            "providerId": provider_id,
            "state": "resolved",
            "scope": {
                "schemaId": "agent.semantic-protocols.project-resolution",
                "schemaVersion": "1",
                "state": "resolved",
                "completeness": "exact",
                "projectIdentity": {
                    "projectEntry": project_entry
                },
                "resolvedSourceScopes": [{
                    "roots": ["."],
                    "explicitPaths": [],
                    "extensions": [source_extension],
                    "includeAuthority": "package-manager",
                    "exclusions": []
                }],
                "resolutionGeneration": format!("test-{language_id}-project-resolution")
            }
        })
        .to_string();
        let guide_marker = match binary {
            "rs-harness" => {
                "[agent-guide] runtime=agent-semantic-hook language=rust provider=rs-harness"
            }
            "ts-harness" => "[ts-harness-guide]",
            "py-harness" | "custom-py-harness" => "[py-harness-guide]",
            _ => "[agent-guide]",
        };
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"guide\" ]; then\n  printf '%s\\n' '{}'\n  exit 0\nfi\nif [ \"$1\" = \"project-resolution-stdin\" ]; then\n  printf '%s\\n' '{}'\n  exit 0\nfi\nexit 0\n",
            guide_marker, project_resolution
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
            "schemaId = \"asp.provider-install-lock.v1\"\nlanguage = \"{language_id}\"\nprovider = \"{provider_id}\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{entrypoint_digest}\"\ninstalledEntrypointMetadataDigest = \"{metadata_digest}\"\n",
            path.display()
        ),
    )
    .expect("write provider install receipt");
    std::fs::canonicalize(&path).unwrap_or(path)
}

pub(super) fn rust_harness_activation() -> HookRuntime {
    parse_hook_activation(&root_owned_rust_activation_json())
        .expect("valid root-owned rust activation")
}
