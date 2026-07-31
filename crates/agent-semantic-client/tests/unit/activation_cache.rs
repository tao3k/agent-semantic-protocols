use agent_semantic_client_core::{ASP_PROVIDER_ACTIVATION_PATH_ENV, LanguageId};
use agent_semantic_hook::{build_default_activation, write_activation};

use crate::test_support::{CACHE_TEST_LOCK, EnvVarGuard};

#[test]
fn activation_loader_does_not_reintroduce_legacy_provider_command_prefix() {
    let _guard = CACHE_TEST_LOCK.lock().expect("cache test lock");
    let root = temp_project_root("activation-cache-refresh");
    let runtime_bin = root.join(".asp-state/runtime/bin");
    let provider_v1 = runtime_bin.join("provider-v1");
    let provider_v2 = runtime_bin.join("provider-v2");
    std::fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"activation-cache-fixture\"\nversion = \"0.1.0\"\n",
    )
    .expect("write Python project entry");
    std::fs::create_dir_all(root.join("src")).expect("create Python source root");
    std::fs::write(root.join("src/app.py"), "def run():\n    return True\n")
        .expect("write Python source");
    crate::cache_cli_source_index_tests::fixtures::write_project_resolution_provider(
        &provider_v1,
        "python",
        "py-harness",
        ".py",
        &["src"],
        &[],
    );
    crate::cache_cli_source_index_tests::fixtures::write_project_resolution_provider(
        &provider_v2,
        "python",
        "py-harness",
        ".py",
        &["src"],
        &[],
    );
    let _state_home = EnvVarGuard::set("ASP_STATE_HOME", root.join(".asp-state"));
    let _ignored_cache_home = EnvVarGuard::set("PRJ_CACHE_HOME", root.join(".cache-home"));
    let activation_path = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("hooks")
        .join("activation.json");

    write_python_provider_config(&root, "provider-v1");
    write_python_provider_install_receipt(&root, &provider_v1);
    let activation = build_default_activation(&root).expect("build initial activation");
    write_activation(&activation_path, &activation).expect("write initial activation");
    let initial_activation = std::fs::read(&activation_path).expect("read initial activation");
    write_python_provider_config(&root, "provider-v2");
    write_python_provider_install_receipt(&root, &provider_v2);

    let _activation_path = EnvVarGuard::set(ASP_PROVIDER_ACTIVATION_PATH_ENV, &activation_path);

    let snapshot = crate::activation_cache::load_provider_registry_snapshot(&root, &root, true)
        .expect("snapshot");

    let provider = snapshot
        .provider_for_language(&LanguageId::from("python"))
        .expect("python provider");
    assert_eq!(provider.binary, "provider-v1");
    assert!(provider.provider_command_prefix.is_empty());
    assert_eq!(
        std::fs::read(&activation_path).expect("read activation after load"),
        initial_activation,
        "the client loader must not rewrite daemon-owned activation state"
    );
    assert!(!root.join(".cache-home").exists());
    let _ = std::fs::remove_dir_all(root);
}

fn temp_project_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("asp-client-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp project root");
    let status = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&root)
        .status()
        .expect("initialize Git fixture");
    assert!(status.success(), "initialize Git fixture");
    root
}

fn write_python_provider_config(root: &std::path::Path, binary: &str) {
    crate::test_support::write_hermetic_provider_registry_config(root, "python", binary);
}

fn write_python_provider_install_receipt(root: &std::path::Path, binary: &std::path::Path) {
    crate::test_support::write_hermetic_provider_install_receipt(root, "python", binary);
}
