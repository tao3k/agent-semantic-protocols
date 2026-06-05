use agent_semantic_hook::{
    RuntimeProviderHealthStatus, load_runtime_profiles, parse_hook_activation,
};

use crate::rust_harness_activation::support::{
    write_failing_provider_binary, write_fake_provider_binary, write_fake_provider_file,
};

use super::support::{git_project_root, protocol_command};

#[test]
fn cli_install_uses_static_provider_manifest_without_running_guide() {
    let root = git_project_root("install-static-provider-manifest");
    let provider_path = write_failing_provider_binary(&root, "py-harness");
    let output = protocol_command()
        .env("PATH", &provider_path)
        .env("CODEX_HOME", root.join(".codex-home"))
        .args([
            "hook",
            "install",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run agent-semantic-protocol install");
    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let activation =
        std::fs::read_to_string(root.join(".cache/agent-semantic-protocol/hooks/activation.json"))
            .expect("installed activation");
    let registry = parse_hook_activation(&activation).expect("valid installed activation");
    assert_eq!(registry.providers.len(), 1);
    assert_eq!(registry.providers[0].language_id, "python");
    let runtime_profiles =
        load_runtime_profiles(&root.join(".cache/agent-semantic-protocol/runtime/profiles.json"))
            .expect("valid runtime profiles");
    assert_eq!(runtime_profiles.providers.len(), 1);
    assert_eq!(runtime_profiles.providers[0].language_id, "python");
    assert_eq!(
        runtime_profiles.providers[0].health.status,
        RuntimeProviderHealthStatus::Available
    );
    let resolved_binary = runtime_profiles.providers[0]
        .resolved_binary
        .as_deref()
        .expect("resolved provider binary");
    assert!(resolved_binary.ends_with("/.bin/py-harness"));
    assert!(std::path::Path::new(resolved_binary).is_file());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cli_install_runtime_profile_prefers_project_bin_provider() {
    let root = git_project_root("install-project-bin-provider");
    let external_root = git_project_root("install-external-provider");
    let project_provider_path = write_fake_provider_binary(&root, "py-harness");
    let external_provider_path = write_fake_provider_file(&external_root, "py-harness", 0o755);
    let path = std::env::join_paths([external_provider_path, project_provider_path])
        .expect("provider path");
    let output = protocol_command()
        .env("PATH", path)
        .env("CODEX_HOME", root.join(".codex-home"))
        .args([
            "hook",
            "install",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run agent-semantic-protocol install");
    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let runtime_profiles =
        load_runtime_profiles(&root.join(".cache/agent-semantic-protocol/runtime/profiles.json"))
            .expect("valid runtime profiles");
    let resolved_binary = runtime_profiles.providers[0]
        .resolved_binary
        .as_deref()
        .expect("resolved provider binary");
    let project_bin = std::fs::canonicalize(root.join(".bin")).expect("canonical project bin");
    assert!(
        std::path::Path::new(resolved_binary).starts_with(&project_bin),
        "expected runtime profile to prefer {}, got {resolved_binary}",
        project_bin.display()
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&external_root);
}

#[test]
fn cli_install_asp_toml_can_disable_language_and_override_provider_binary() {
    let root = git_project_root("install-asp-toml-provider-config");
    let empty_path = root.join("empty-path");
    std::fs::create_dir_all(&empty_path).expect("empty path");
    write_fake_provider_file(&root, "custom-py-harness", 0o755);
    write_fake_provider_file(&root, "ts-harness", 0o755);
    std::fs::write(
        root.join("asp.toml"),
        r#"
[providers.rust]
enabled = false

[providers.typescript]
enabled = false

[providers.python]
binary = ".bin/custom-py-harness"

[providers.julia]
enabled = false
"#,
    )
    .expect("write asp.toml");

    let output = protocol_command()
        .env("PATH", &empty_path)
        .env("CODEX_HOME", root.join(".codex-home"))
        .args([
            "hook",
            "install",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run agent-semantic-protocol install");
    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let activation =
        std::fs::read_to_string(root.join(".cache/agent-semantic-protocol/hooks/activation.json"))
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

    let runtime_profiles =
        load_runtime_profiles(&root.join(".cache/agent-semantic-protocol/runtime/profiles.json"))
            .expect("valid runtime profiles");
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
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cli_install_writes_executable_python_ingest_route() {
    let root = git_project_root("install-python");
    let provider_path = write_fake_provider_binary(&root, "py-harness");
    let output = protocol_command()
        .env("PATH", &provider_path)
        .env("CODEX_HOME", root.join(".codex-home"))
        .args([
            "hook",
            "install",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let activation =
        std::fs::read_to_string(root.join(".cache/agent-semantic-protocol/hooks/activation.json"))
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
            "--view",
            "seeds",
            "{projectRoot}"
        ]
    );
    let _ = std::fs::remove_dir_all(&root);
}
