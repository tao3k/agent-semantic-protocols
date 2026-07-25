use agent_semantic_hook::{
    HookActivation, build_default_activation, load_or_sync_activation, write_activation,
};
use std::fs;

use super::temp_root;

#[test]
fn generated_activation_sync_refreshes_stale_manifest_coverage_defaults() {
    let _state_home_lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = temp_root("stale-coverage-defaults");
    let state_home = root.join(".asp-state-home");
    let _state_home_guard = super::activation_bin::StateHomeEnvGuard::set(&state_home);
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n",
    )
    .expect("write cargo manifest");
    super::install_state_home_provider(&state_home, "rust", "rs-harness", "rs-harness");

    let activation_path = test_activation_path(&root, &root);
    let mut activation = build_default_activation(&root).expect("build activation");
    let rust_provider = activation
        .providers
        .iter_mut()
        .find(|provider| provider.language_id == "rust")
        .expect("rust provider");
    rust_provider.coverage.ignored_path_prefixes = vec!["target".to_string()];
    write_activation(&activation_path, &activation).expect("write stale activation");

    let runtime = load_or_sync_activation(&activation_path, &root).expect("sync activation");
    let runtime_rust_provider = runtime
        .providers
        .iter()
        .find(|provider| provider.language_id == "rust")
        .expect("runtime rust provider");
    assert!(
        runtime_rust_provider
            .ignored_path_prefixes
            .iter()
            .any(|prefix| prefix == ".data"),
        "runtime should refresh common ignored prefixes"
    );

    let refreshed_text = fs::read_to_string(&activation_path).expect("read activation");
    let refreshed: HookActivation =
        serde_json::from_str(&refreshed_text).expect("parse activation");
    let refreshed_rust_provider = refreshed
        .providers
        .iter()
        .find(|provider| provider.language_id == "rust")
        .expect("refreshed rust provider");
    assert!(
        refreshed_rust_provider
            .coverage
            .ignored_path_prefixes
            .iter()
            .any(|prefix| prefix == ".cache")
    );
    assert!(
        refreshed_rust_provider
            .coverage
            .ignored_path_prefixes
            .iter()
            .any(|prefix| prefix == ".data")
    );

    fs::remove_dir_all(root).expect("remove temp root");
}

fn test_activation_path(
    project_root: &std::path::Path,
    state_root: &std::path::Path,
) -> std::path::PathBuf {
    let resolved = agent_semantic_runtime::state_core::ResolvedState::resolve_with_state_home(
        project_root,
        state_root.join(".agent-semantic-protocols"),
    )
    .expect("resolve test state");
    std::fs::create_dir_all(&resolved.paths.workspace_dir).expect("create workspace state dir");
    std::fs::write(
        &resolved.paths.workspace_json,
        serde_json::to_string(&serde_json::json!({
            "root": project_root.display().to_string()
        }))
        .expect("serialize workspace manifest"),
    )
    .expect("write workspace manifest");
    resolved
        .paths
        .hooks_dir
        .join("state")
        .join("activation.json")
}
