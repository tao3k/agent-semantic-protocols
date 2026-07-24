use agent_semantic_hook::{
    HookActivation, build_default_activation, load_or_sync_activation, write_activation,
};
use std::fs;

use super::{make_executable, temp_root};

#[test]
fn generated_activation_sync_refreshes_newly_available_parent_workspace_provider() {
    let root = temp_root("nested-gerbil-refresh-parent-bin-provider");
    let child = root
        .join("languages")
        .join("gerbil-scheme-language-project-harness");
    fs::create_dir_all(root.join(".bin")).expect("create workspace bin");
    fs::create_dir_all(child.join("src")).expect("create child src");
    fs::write(child.join("gerbil.pkg"), "(package: sample/gerbil)\n").expect("write gerbil.pkg");
    write_agent_config(&root, "[providers]\n");
    write_agent_config(&child, "[providers.gerbil-scheme]\nenabled = false\n");
    let asp_bin = root.join(".bin/asp");
    fs::write(&asp_bin, "#!/bin/sh\nexit 0\n").expect("write asp bin");
    make_executable(&asp_bin);
    let activation_path = test_activation_path(&child, &root);
    let initial_activation = build_default_activation(&child).expect("build initial activation");
    assert!(
        !initial_activation
            .providers
            .iter()
            .any(|provider| provider.language_id == "gerbil-scheme")
    );
    write_activation(&activation_path, &initial_activation).expect("write old activation");

    fs::remove_file(child.join(".agents").join("asp.toml"))
        .expect("enable default child providers");
    let gerbil_bin = root.join(".bin/gslph");
    fs::write(&gerbil_bin, "#!/bin/sh\nexit 0\n").expect("write gerbil provider bin");
    make_executable(&gerbil_bin);

    let runtime = load_or_sync_activation(&activation_path, &child).expect("sync activation");
    assert!(
        runtime
            .providers
            .iter()
            .any(|provider| provider.language_id == "gerbil-scheme"),
        "generated activation should refresh when a parent workspace Gerbil provider becomes available"
    );
    let refreshed_activation = fs::read_to_string(&activation_path).expect("read refreshed");
    assert!(refreshed_activation.contains("\"languageId\": \"gerbil-scheme\""));

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn generated_activation_sync_refreshes_stale_manifest_coverage_defaults() {
    let _home_lock = super::HOME_ENV_LOCK.lock().expect("lock HOME env");
    let root = temp_root("stale-coverage-defaults");
    fs::create_dir_all(root.join(".bin")).expect("create bin dir");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n",
    )
    .expect("write cargo manifest");
    let rs_harness = root.join(".bin/rs-harness");
    fs::write(&rs_harness, "#!/bin/sh\nexit 0\n").expect("write rust provider bin");
    make_executable(&rs_harness);
    let gslph = root.join(".bin/gslph");
    fs::write(&gslph, "#!/bin/sh\nexit 0\n").expect("write Gerbil provider bin");
    make_executable(&gslph);

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
        .workspace_dir
        .join("live")
        .join("hooks")
        .join("state")
        .join("activation.json")
}

fn write_agent_config(root: &std::path::Path, contents: &str) {
    let config_path = root.join(".agents").join("asp.toml");
    fs::create_dir_all(config_path.parent().expect("agent config parent"))
        .expect("create agent config parent");
    fs::write(&config_path, contents).expect("write .agents/asp.toml");
}
