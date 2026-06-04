use agent_semantic_hook::parse_hook_activation;

use crate::rust_harness_activation::support::{
    write_failing_provider_binary, write_fake_provider_binary,
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
