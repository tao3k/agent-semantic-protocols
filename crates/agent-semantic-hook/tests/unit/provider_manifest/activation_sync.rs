use agent_semantic_hook::{
    HookActivation, build_default_activation, load_or_sync_activation,
    materialize_active_asp_artifact_receipt_for_current_process,
    verify_active_asp_artifact_receipt, write_activation,
};
use std::fs;

use super::temp_root;

#[test]
fn generated_activation_sync_refreshes_stale_manifest_coverage_defaults() {
    let _state_home_lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = temp_root("stale-coverage-defaults");
    super::git_init(&root);
    fs::create_dir_all(root.join("src")).expect("create Rust source root");
    let state_home = root.join(".asp-state-home");
    let _state_home_guard = super::activation_bin::StateHomeEnvGuard::set(&state_home);
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n",
    )
    .expect("write cargo manifest");
    fs::write(root.join("src/lib.rs"), "pub fn fixture() {}\n").expect("write Rust candidate");
    super::install_state_home_provider(&state_home, "rust", "rs-harness", "rs-harness");

    let activation_path = test_activation_path(&root, &root);
    let mut activation = build_default_activation(&root).expect("build activation");
    let rust_provider = activation
        .providers
        .iter_mut()
        .find(|provider| provider.language_id == "rust")
        .expect("rust provider");
    let expected_source_extensions = rust_provider.coverage.source_extensions.clone();
    let expected_config_files = rust_provider.coverage.config_files.clone();
    rust_provider.coverage.source_extensions.clear();
    rust_provider.coverage.config_files.clear();
    write_activation(&activation_path, &activation).expect("write stale activation");

    let runtime = load_or_sync_activation(&activation_path, &root).expect("sync activation");
    let runtime_rust_provider = runtime
        .providers
        .iter()
        .find(|provider| provider.language_id == "rust")
        .expect("runtime rust provider");
    assert_eq!(
        runtime_rust_provider.source_extensions, expected_source_extensions,
        "runtime should resolve source extensions from the current manifest"
    );
    assert_eq!(
        runtime_rust_provider.config_files, expected_config_files,
        "runtime should resolve config files from the current manifest"
    );

    let refreshed_text = fs::read_to_string(&activation_path).expect("read activation");
    let refreshed: HookActivation =
        serde_json::from_str(&refreshed_text).expect("parse activation");
    let refreshed_rust_provider = refreshed
        .providers
        .iter()
        .find(|provider| provider.language_id == "rust")
        .expect("refreshed rust provider");
    assert_eq!(
        refreshed_rust_provider.coverage.source_extensions, expected_source_extensions,
        "activation sync should durably refresh manifest source extensions"
    );
    assert_eq!(
        refreshed_rust_provider.coverage.config_files, expected_config_files,
        "activation sync should durably refresh manifest config files"
    );

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn generated_activation_with_unknown_field_and_valid_receipt_rebuilds() {
    let _state_home_lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = temp_root("generated-unknown-field");
    super::git_init(&root);
    fs::create_dir_all(root.join("src")).expect("create Rust source root");
    let state_home = root.join(".asp-state-home");
    let _state_home_guard = super::activation_bin::StateHomeEnvGuard::set(&state_home);
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n",
    )
    .expect("write cargo manifest");
    fs::write(root.join("src/lib.rs"), "pub fn fixture() {}\n").expect("write Rust candidate");
    super::install_state_home_provider(&state_home, "rust", "rs-harness", "rs-harness");

    let activation_path = test_activation_path(&root, &root);
    let runtime =
        load_or_sync_activation(&activation_path, &root).expect("create generated activation");
    let mut activation_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&activation_path).expect("read generated activation"),
    )
    .expect("parse generated activation value");
    activation_json
        .as_object_mut()
        .expect("activation object")
        .insert(
            "futureGeneratedField".to_string(),
            serde_json::json!({"version": 2}),
        );
    fs::write(
        &activation_path,
        serde_json::to_string_pretty(&activation_json).expect("serialize future activation"),
    )
    .expect("write future generated activation");
    materialize_active_asp_artifact_receipt_for_current_process(&activation_path, &runtime)
        .expect("materialize valid receipt for future generated activation");
    let current_exe = std::env::current_exe().expect("current test executable");
    verify_active_asp_artifact_receipt(&activation_path, &[&current_exe])
        .expect("future generated activation receipt should be valid");

    load_or_sync_activation(&activation_path, &root)
        .expect("unknown generated field should trigger atomic rebuild");
    let refreshed_text = fs::read_to_string(&activation_path).expect("read rebuilt activation");
    let refreshed_value: serde_json::Value =
        serde_json::from_str(&refreshed_text).expect("parse rebuilt activation");
    assert!(
        refreshed_value.get("futureGeneratedField").is_none(),
        "atomic rebuild should replace the incompatible generated activation"
    );
    serde_json::from_str::<HookActivation>(&refreshed_text)
        .expect("rebuilt activation should match the current strict schema");

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn non_generated_activation_with_unknown_field_fails_closed() {
    let _state_home_lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = temp_root("custom-unknown-field");
    super::git_init(&root);
    fs::create_dir_all(root.join("src")).expect("create Rust source root");
    let state_home = root.join(".asp-state-home");
    let _state_home_guard = super::activation_bin::StateHomeEnvGuard::set(&state_home);
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n",
    )
    .expect("write cargo manifest");
    fs::write(root.join("src/lib.rs"), "pub fn fixture() {}\n").expect("write Rust candidate");
    super::install_state_home_provider(&state_home, "rust", "rs-harness", "rs-harness");
    let activation_path = root.join("custom-activation.json");
    let activation = build_default_activation(&root).expect("build activation");
    let mut activation_json =
        serde_json::to_value(activation).expect("serialize custom activation value");
    activation_json
        .as_object_mut()
        .expect("activation object")
        .insert("futureUserField".to_string(), serde_json::json!(true));
    fs::write(
        &activation_path,
        serde_json::to_string_pretty(&activation_json).expect("serialize custom activation"),
    )
    .expect("write custom activation");

    let error = load_or_sync_activation(&activation_path, &root)
        .expect_err("non-generated activation must remain fail-closed");
    assert!(
        error.contains("unknown field `futureUserField`"),
        "unexpected error: {error}"
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
