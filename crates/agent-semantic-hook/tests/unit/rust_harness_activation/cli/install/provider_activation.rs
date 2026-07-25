use agent_semantic_hook::parse_hook_activation;
use std::env;

use crate::rust_harness_activation::support::{
    asp_bin_dir, write_failing_state_home_provider_binary,
    write_home_local_unmanaged_provider_file, write_state_home_provider_binary,
    write_unmanaged_provider_file,
};

use super::support::{
    codex_plugin_install_args, git_project_root, protocol_command, sync_test_state, test_host_path,
};

#[test]
fn cli_install_uses_static_provider_manifest_without_running_guide() {
    let root = git_project_root("install-static-provider-manifest");
    let asp_state_home = root.join(".asp-state-home");
    write_failing_state_home_provider_binary(&asp_state_home, "python", "py-harness", "py-harness");
    sync_test_state(&root, &asp_state_home);
    let asp_bin_dir = asp_bin_dir();
    let host_path = test_host_path(&root, &asp_bin_dir);
    let output = protocol_command()
        .env("PATH", &host_path)
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
    let activation = std::fs::read_to_string(installed_activation_path(&asp_state_home))
        .expect("installed activation");
    let registry = parse_hook_activation(&activation).expect("valid installed activation");
    let python = registry
        .providers
        .iter()
        .find(|provider| provider.language_id == "python")
        .expect("python provider");
    assert!(
        python.provider_command_prefix.is_empty(),
        "installed State Home v1 activation must not persist a provider path"
    );
    assert!(
        !root
            .join(".cache/agent-semantic-protocol/runtime/profiles.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cli_install_runtime_profile_uses_state_home_provider_only() {
    let root = git_project_root("install-state-home-provider");
    let asp_state_home = root.join(".asp-state-home");
    let external_root = git_project_root("install-external-provider");
    write_state_home_provider_binary(&asp_state_home, "python", "py-harness", "py-harness");
    sync_test_state(&root, &asp_state_home);
    let project_provider_path = write_unmanaged_provider_file(&root, "py-harness", 0o755);
    let external_provider_path = write_unmanaged_provider_file(&external_root, "py-harness", 0o755);
    write_home_local_unmanaged_provider_file(&external_root, "py-harness", 0o755);
    let asp_bin_dir = asp_bin_dir();
    let path = std::env::join_paths([
        external_provider_path.as_path(),
        project_provider_path.as_path(),
        asp_bin_dir.as_path(),
    ])
    .expect("provider and asp PATH");
    let output = protocol_command()
        .env("PATH", path)
        .env("SEMANTIC_AGENT_BIN_DIR", &asp_bin_dir)
        .env("ASP_STATE_HOME", &asp_state_home)
        .env("HOME", &external_root)
        .env("CODEX_HOME", root.join(".codex-home"))
        .args(codex_plugin_install_args(&root))
        .output()
        .expect("run agent-semantic-protocol install");
    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let activation = std::fs::read_to_string(installed_activation_path(&asp_state_home))
        .expect("installed activation");
    let registry = parse_hook_activation(&activation).expect("valid installed activation");
    let python = registry
        .providers
        .iter()
        .find(|provider| provider.language_id == "python")
        .expect("python provider");
    assert!(
        python.provider_command_prefix.is_empty(),
        "runtime provider resolution belongs to the receipt-validated State Home profile"
    );
    assert!(
        !root
            .join(".cache/agent-semantic-protocol/runtime/profiles.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&external_root);
}

#[test]
fn cli_install_rejects_project_and_path_provider_without_state_home_receipt() {
    let root = git_project_root("install-reject-unmanaged-provider");
    let asp_state_home = root.join(".asp-state-home");
    let external_root = git_project_root("install-reject-path-provider");
    let managed_provider =
        write_state_home_provider_binary(&asp_state_home, "python", "py-harness", "py-harness");
    sync_test_state(&root, &asp_state_home);
    let activation_path = installed_activation_path(&asp_state_home);
    let activation_before =
        std::fs::read_to_string(&activation_path).expect("activation created by explicit sync");
    std::fs::remove_file(managed_provider).expect("remove managed provider after State sync");
    std::fs::remove_file(asp_state_home.join("runtime/provider-locks/python.lock.toml"))
        .expect("remove provider install receipt after State sync");
    let project_provider_path = write_unmanaged_provider_file(&root, "py-harness", 0o755);
    let external_provider_path = write_unmanaged_provider_file(&external_root, "py-harness", 0o755);
    write_home_local_unmanaged_provider_file(&external_root, "py-harness", 0o755);
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
        .env("HOME", &external_root)
        .env("CODEX_HOME", root.join(".codex-home"))
        .args(codex_plugin_install_args(&root))
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("expected State Home runtime bin to contain at least one executable"),
        "{stderr}"
    );
    assert_eq!(
        std::fs::read_to_string(&activation_path).expect("activation survives rejected install"),
        activation_before,
        "failed install must not mutate the last explicitly synced activation"
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&external_root);
}

#[test]
fn cli_install_asp_toml_can_select_state_home_provider_basename() {
    let root = git_project_root("install-asp-toml-provider-config");
    let asp_state_home = root.join(".asp-state-home");
    let empty_path = root.join("empty-path");
    std::fs::create_dir_all(&empty_path).expect("empty path");
    write_state_home_provider_binary(&asp_state_home, "python", "py-harness", "custom-py-harness");
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
    sync_test_state(&root, &asp_state_home);

    let asp_bin_dir = asp_bin_dir();
    let path = env::join_paths([root.join(".bin"), empty_path, asp_bin_dir.to_path_buf()])
        .expect("join host and protocol PATH");
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
        "custom logical basename must still resolve only through State Home at runtime"
    );

    assert!(
        !root
            .join(".cache/agent-semantic-protocol/runtime/profiles.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cli_install_writes_executable_python_ingest_route() {
    let root = git_project_root("install-python");
    let asp_state_home = root.join(".asp-state-home");
    write_state_home_provider_binary(&asp_state_home, "python", "py-harness", "py-harness");
    sync_test_state(&root, &asp_state_home);
    let asp_bin_dir = asp_bin_dir();
    let host_path = test_host_path(&root, &asp_bin_dir);
    let output = protocol_command()
        .env("PATH", &host_path)
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
