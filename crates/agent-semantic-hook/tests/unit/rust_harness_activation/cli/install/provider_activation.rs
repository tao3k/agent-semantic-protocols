use agent_semantic_hook::{
    RuntimeProviderHealthStatus, parse_hook_activation,
    runtime_profiles_for_runtime_with_state_home,
};
use std::env;

use crate::rust_harness_activation::support::{
    asp_bin_dir, write_failing_state_home_provider_binary, write_state_home_provider_binary,
    write_unmanaged_provider_file,
};

use super::support::{codex_plugin_install_args, git_project_root, protocol_command};

#[test]
fn hook_materializes_static_provider_manifest_after_cli_install_without_running_guide() {
    let root = git_project_root("install-static-provider-manifest");
    let asp_state_home = root.join(".asp-state-home");
    let provider_bin = write_failing_state_home_provider_binary(
        &asp_state_home,
        "python",
        "py-harness",
        "py-harness",
    );
    let asp_bin_dir = asp_bin_dir();
    let protocol_bin_dir = root.join(".agent-bin");
    let path = env::join_paths([protocol_bin_dir.as_path(), asp_bin_dir.as_path()])
        .expect("protocol and ASP PATH");
    let output = protocol_command()
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &asp_bin_dir)
        .env("ASP_STATE_HOME", &asp_state_home)
        .env("CODEX_HOME", root.join(".codex-home"))
        .args(codex_plugin_install_args(&root))
        .output()
        .expect("run agent-semantic-protocol install");
    assert!(
        output.status.success(),
        "root={} install stdout={} stderr={}",
        root.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    materialize_hook_activation(&root, &asp_state_home, &path, &asp_bin_dir);
    let activation = std::fs::read_to_string(installed_activation_path(&asp_state_home))
        .expect("installed activation");
    let registry = parse_hook_activation(&activation).expect("valid installed activation");
    assert!(
        registry
            .providers
            .iter()
            .any(|provider| provider.language_id == "python")
    );
    let runtime_profiles =
        runtime_profiles_for_runtime_with_state_home(&root, &asp_state_home, &registry)
            .expect("runtime profiles with explicit State Home");
    let python_profile = runtime_profiles
        .providers
        .iter()
        .find(|provider| provider.language_id == "python")
        .expect("python profile");
    assert_eq!(
        python_profile.health.status,
        RuntimeProviderHealthStatus::Available
    );
    let resolved_binary = python_profile
        .resolved_binary
        .as_deref()
        .unwrap_or_else(|| panic!("resolved provider binary: profile={python_profile:?}"));
    assert_eq!(resolved_binary, provider_bin.display().to_string());
    assert!(
        !root
            .join(".cache/agent-semantic-protocol/runtime/profiles.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn hook_materialized_runtime_profile_uses_state_home_provider_only() {
    let root = git_project_root("install-state-home-provider");
    let asp_state_home = root.join(".asp-state-home");
    let external_root = git_project_root("install-external-provider");
    let state_home_provider =
        write_state_home_provider_binary(&asp_state_home, "python", "py-harness", "py-harness");
    let project_provider_path = write_unmanaged_provider_file(&root, "py-harness", 0o755);
    let external_provider_path = write_unmanaged_provider_file(&external_root, "py-harness", 0o755);
    let asp_bin_dir = asp_bin_dir();
    let protocol_bin_dir = root.join(".agent-bin");
    let path = std::env::join_paths([
        protocol_bin_dir.as_path(),
        external_provider_path.as_path(),
        project_provider_path.as_path(),
        asp_bin_dir.as_path(),
    ])
    .expect("provider and asp PATH");
    let output = protocol_command()
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &asp_bin_dir)
        .env("ASP_STATE_HOME", &asp_state_home)
        .env("CODEX_HOME", root.join(".codex-home"))
        .args(codex_plugin_install_args(&root))
        .output()
        .expect("run agent-semantic-protocol install");
    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    materialize_hook_activation(&root, &asp_state_home, &path, &asp_bin_dir);
    let activation = std::fs::read_to_string(installed_activation_path(&asp_state_home))
        .expect("installed activation");
    let registry = parse_hook_activation(&activation).expect("valid installed activation");
    let runtime_profiles =
        runtime_profiles_for_runtime_with_state_home(&root, &asp_state_home, &registry)
            .expect("runtime profiles with explicit State Home");
    let python_profile = runtime_profiles
        .providers
        .iter()
        .find(|provider| provider.language_id == "python")
        .expect("python runtime profile");
    let resolved_binary = python_profile
        .resolved_binary
        .as_deref()
        .unwrap_or_else(|| panic!("resolved provider binary: profile={python_profile:?}"));
    assert_eq!(resolved_binary, state_home_provider.display().to_string());
    assert!(
        !root
            .join(".cache/agent-semantic-protocol/runtime/profiles.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&external_root);
}

#[test]
fn agent_config_sync_is_provider_independent_and_does_not_materialize_activation() {
    let root = git_project_root("install-reject-unmanaged-provider");
    let asp_state_home = root.join(".asp-state-home");
    let external_root = git_project_root("install-reject-path-provider");
    let project_provider_path = write_unmanaged_provider_file(&root, "py-harness", 0o755);
    let external_provider_path = write_unmanaged_provider_file(&external_root, "py-harness", 0o755);
    let asp_bin_dir = asp_bin_dir();
    let path = std::env::join_paths([
        external_provider_path.as_path(),
        project_provider_path.as_path(),
        asp_bin_dir.as_path(),
    ])
    .expect("unmanaged providers and asp PATH");

    let output = protocol_command()
        .env("PATH", path)
        .env("SEMANTIC_AGENT_BIN_DIR", &asp_bin_dir)
        .env("ASP_STATE_HOME", &asp_state_home)
        .args(["agent", "config", "sync"])
        .output()
        .expect("run agent-semantic-protocol agent config sync");

    assert!(
        output.status.success(),
        "sync stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("orgStateSync=consumer-lazy") && stdout.contains("activationWrites=0"),
        "{stdout}"
    );
    let mut activation_paths = Vec::new();
    collect_activation_paths(&asp_state_home, &mut activation_paths);
    assert!(
        activation_paths.is_empty(),
        "failed sync must not materialize activation: {activation_paths:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&external_root);
}

#[test]
fn hook_materialization_honors_asp_toml_state_home_provider_basename() {
    let root = git_project_root("install-asp-toml-provider-config");
    let asp_state_home = root.join(".asp-state-home");
    let empty_path = root.join("empty-path");
    std::fs::create_dir_all(&empty_path).expect("empty path");
    let custom_provider = write_state_home_provider_binary(
        &asp_state_home,
        "python",
        "py-harness",
        "custom-py-harness",
    );
    let config_path = root.join(".agents").join("asp.toml");
    std::fs::create_dir_all(config_path.parent().expect("agent config parent"))
        .expect("create agent config parent");
    std::fs::write(
        &config_path,
        r#"
[providers.rust]
enabled = false

[providers.typescript]
enabled = false

[providers.python]
binary = "custom-py-harness"

[providers.julia]
enabled = false

[providers.gerbil-scheme]
enabled = false

[providers.org]
enabled = false

[providers.md]
enabled = false
"#,
    )
    .expect("write .agents/asp.toml");

    let asp_bin_dir = asp_bin_dir();
    let protocol_bin_dir = root.join(".agent-bin");
    write_real_asp_launcher(&protocol_bin_dir);
    let path = env::join_paths([
        protocol_bin_dir.as_path(),
        empty_path.as_path(),
        asp_bin_dir.as_path(),
    ])
    .expect("join PATH");
    let output = protocol_command()
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &asp_bin_dir)
        .env("ASP_STATE_HOME", &asp_state_home)
        .env("CODEX_HOME", root.join(".codex-home"))
        .args(codex_plugin_install_args(&root))
        .output()
        .expect("run agent-semantic-protocol install");
    assert!(
        output.status.success(),
        "root={} install stdout={} stderr={}",
        root.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    materialize_hook_activation(&root, &asp_state_home, &path, &asp_bin_dir);
    let activation = std::fs::read_to_string(installed_activation_path(&asp_state_home))
        .expect("installed activation");
    let registry = parse_hook_activation(&activation).expect("valid installed activation");
    assert_eq!(registry.providers.len(), 1);
    let python = registry
        .providers
        .iter()
        .find(|provider| provider.language_id == "python")
        .expect("python provider");
    assert_eq!(python.binary, "custom-py-harness");
    assert!(
        python.provider_command_prefix.is_empty(),
        "State Home v1 activation must persist only the logical basename"
    );
    let runtime_profiles =
        runtime_profiles_for_runtime_with_state_home(&root, &asp_state_home, &registry)
            .expect("runtime profiles with explicit State Home");
    let python_profile = runtime_profiles
        .providers
        .iter()
        .find(|provider| provider.language_id == "python")
        .expect("python runtime profile");
    let expected_binary = custom_provider.display().to_string();
    assert_eq!(
        python_profile.resolved_binary.as_deref(),
        Some(expected_binary.as_str())
    );
    assert_eq!(
        python_profile.resolved_binary.as_deref(),
        Some(python_profile.argv[0].as_str())
    );
    assert!(
        python_profile.argv[0] == custom_provider.display().to_string(),
        "{:?}",
        python_profile.argv
    );
    assert_eq!(
        python_profile.health.status,
        RuntimeProviderHealthStatus::Available
    );
    assert!(
        !root
            .join(".cache/agent-semantic-protocol/runtime/profiles.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn hook_materialization_writes_executable_python_ingest_route() {
    let root = git_project_root("install-python");
    let asp_state_home = root.join(".asp-state-home");
    write_state_home_provider_binary(&asp_state_home, "python", "py-harness", "py-harness");
    let asp_bin_dir = asp_bin_dir();
    let protocol_bin_dir = root.join(".agent-bin");
    let path = env::join_paths([protocol_bin_dir.as_path(), asp_bin_dir.as_path()])
        .expect("protocol and ASP PATH");
    let output = protocol_command()
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &asp_bin_dir)
        .env("ASP_STATE_HOME", &asp_state_home)
        .env("CODEX_HOME", root.join(".codex-home"))
        .args(codex_plugin_install_args(&root))
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    materialize_hook_activation(&root, &asp_state_home, &path, &asp_bin_dir);
    let activation = std::fs::read_to_string(installed_activation_path(&asp_state_home))
        .expect("installed activation");
    let registry = parse_hook_activation(&activation).expect("valid installed activation");
    let python = registry
        .providers
        .iter()
        .find(|provider| provider.language_id == "python")
        .expect("python provider");
    assert_eq!(
        python.routes.ingest.argv,
        [
            "py-harness",
            "search",
            "ingest",
            "owner",
            "tests",
            "--workspace",
            "{workspace}",
            "--view",
            "seeds"
        ]
    );
    let _ = std::fs::remove_dir_all(&root);
}

fn installed_activation_path(root: &std::path::Path) -> std::path::PathBuf {
    let mut matches = Vec::new();
    collect_activation_paths(root, &mut matches);
    let expected_project_root = root
        .parent()
        .expect("state home must be rooted under the fixture project")
        .canonicalize()
        .expect("canonical fixture project root");
    // Provider-control-plane activations may coexist with the fixture project
    // under ASP_STATE_HOME. Select the activation owned by this project scope.
    matches.retain(|path| {
        let activation: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(path).expect("read activation candidate"),
        )
        .expect("parse activation candidate");
        activation
            .get("projectRoot")
            .and_then(serde_json::Value::as_str)
            .map(std::path::PathBuf::from)
            .and_then(|project_root| project_root.canonicalize().ok())
            .is_some_and(|project_root| project_root == expected_project_root)
    });
    matches.sort();
    assert_eq!(
        matches.len(),
        1,
        "project activation paths for {expected_project_root:?}: {matches:?}"
    );
    matches.remove(0)
}

fn materialize_hook_activation(
    root: &std::path::Path,
    asp_state_home: &std::path::Path,
    path: &std::ffi::OsStr,
    asp_bin_dir: &std::path::Path,
) {
    let payload = serde_json::json!({
        "cwd": root,
        "hook_event_name": "PostToolUse",
        "session_id": "install-provider-activation-test",
        "tool_name": "Bash",
        "tool_input": {"cmd": "true"},
        "tool_result": {"status": "completed"}
    });
    let mut child = protocol_command()
        .current_dir(root)
        .env("PATH", path)
        .env("SEMANTIC_AGENT_BIN_DIR", asp_bin_dir)
        .env("ASP_STATE_HOME", asp_state_home)
        .env("CODEX_HOME", root.join(".codex-home"))
        .args([
            "hook",
            "--client",
            "codex",
            "post-tool",
            "--emit",
            "decision",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("start post-tool activation materialization");
    std::io::Write::write_all(
        child.stdin.as_mut().expect("post-tool stdin"),
        payload.to_string().as_bytes(),
    )
    .expect("write post-tool payload");
    drop(child.stdin.take());
    let output = child
        .wait_with_output()
        .expect("wait for post-tool activation materialization");
    assert!(
        output.status.success(),
        "post-tool activation materialization failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn collect_activation_paths(dir: &std::path::Path, matches: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_activation_paths(&path, matches);
        } else if path.ends_with("state/activation.json") {
            matches.push(path);
        }
    }
}
use crate::rust_harness_activation::cli::install::support::write_real_asp_launcher;
