use agent_semantic_hook::{
    RuntimeProviderHealthStatus, parse_hook_activation, runtime_profiles_for_runtime,
};
use std::env;

use crate::rust_harness_activation::support::{
    write_failing_provider_binary, write_fake_provider_binary, write_fake_provider_file,
};

use super::support::{codex_plugin_install_args, git_project_root, protocol_command};

#[test]
fn cli_install_uses_static_provider_manifest_without_running_guide() {
    let root = git_project_root("install-static-provider-manifest");
    let asp_state_home = root.join(".asp-state-home");
    let provider_path = write_failing_provider_binary(&root, "py-harness");
    let output = protocol_command()
        .env("PATH", &provider_path)
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
    assert!(
        registry
            .providers
            .iter()
            .any(|provider| provider.language_id == "python")
    );
    assert!(
        registry
            .providers
            .iter()
            .any(|provider| provider.language_id == "org")
    );
    assert!(
        registry
            .providers
            .iter()
            .any(|provider| provider.language_id == "md")
    );
    let runtime_profiles = runtime_profiles_for_runtime(&root, &registry);
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
        .expect("resolved provider binary");
    assert!(resolved_binary.ends_with("/.bin/py-harness"));
    assert!(std::path::Path::new(resolved_binary).is_file());
    assert!(
        !root
            .join(".cache/agent-semantic-protocol/runtime/profiles.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cli_install_runtime_profile_prefers_project_bin_provider() {
    let root = git_project_root("install-project-bin-provider");
    let asp_state_home = root.join(".asp-state-home");
    let external_root = git_project_root("install-external-provider");
    let project_provider_path = write_fake_provider_binary(&root, "py-harness");
    let external_provider_path = write_fake_provider_file(&external_root, "py-harness", 0o755);
    let path = std::env::join_paths([external_provider_path, project_provider_path])
        .expect("provider path");
    let output = protocol_command()
        .env("PATH", path)
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
    let runtime_profiles = runtime_profiles_for_runtime(&root, &registry);
    let python_profile = runtime_profiles
        .providers
        .iter()
        .find(|provider| provider.language_id == "python")
        .expect("python runtime profile");
    let resolved_binary = python_profile
        .resolved_binary
        .as_deref()
        .expect("resolved provider binary");
    let project_bin = std::fs::canonicalize(root.join(".bin")).expect("canonical project bin");
    assert!(
        std::path::Path::new(resolved_binary).starts_with(&project_bin),
        "expected runtime profile to prefer {}, got {resolved_binary}",
        project_bin.display()
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
fn cli_install_asp_toml_can_disable_language_and_override_provider_binary() {
    let root = git_project_root("install-asp-toml-provider-config");
    let asp_state_home = root.join(".asp-state-home");
    let empty_path = root.join("empty-path");
    std::fs::create_dir_all(&empty_path).expect("empty path");
    write_fake_provider_file(&root, "custom-py-harness", 0o755);
    write_fake_provider_file(&root, "ts-harness", 0o755);
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
binary = ".bin/custom-py-harness"

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

    let path = env::join_paths([root.join(".bin"), empty_path]).expect("join PATH");
    let output = protocol_command()
        .env("PATH", &path)
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
    assert_eq!(python.binary, "py-harness");
    assert_eq!(python.provider_command_prefix.len(), 1);
    assert!(
        python.provider_command_prefix[0].ends_with("/.bin/custom-py-harness"),
        "{:?}",
        python.provider_command_prefix
    );

    let runtime_profiles = runtime_profiles_for_runtime(&root, &registry);
    assert_eq!(runtime_profiles.providers.len(), 1);
    let profile = &runtime_profiles.providers[0];
    assert_eq!(profile.language_id, "python");
    assert_eq!(
        profile.resolved_binary.as_deref(),
        Some(profile.argv[0].as_str())
    );
    assert!(
        profile.argv[0].ends_with("/.bin/custom-py-harness"),
        "{:?}",
        profile.argv
    );
    assert_eq!(
        profile.health.status,
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
fn cli_install_writes_executable_python_ingest_route() {
    let root = git_project_root("install-python");
    let asp_state_home = root.join(".asp-state-home");
    let provider_path = write_fake_provider_binary(&root, "py-harness");
    let output = protocol_command()
        .env("PATH", &provider_path)
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
            "items",
            "tests",
            "--workspace",
            "{projectRoot}",
            "--view",
            "seeds"
        ]
    );
    let _ = std::fs::remove_dir_all(&root);
}

fn installed_activation_path(root: &std::path::Path) -> std::path::PathBuf {
    let mut matches = Vec::new();
    collect_activation_paths(root, &mut matches);
    matches.sort();
    assert_eq!(matches.len(), 1, "activation paths: {matches:?}");
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
        } else if path.ends_with("live/hooks/state/activation.json") {
            matches.push(path);
        }
    }
}
