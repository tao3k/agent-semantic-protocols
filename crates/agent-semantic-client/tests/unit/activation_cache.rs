use agent_semantic_client_core::{ASP_PROVIDER_ACTIVATION_PATH_ENV, LanguageId};
use agent_semantic_client_db::ClientDbEngine;
use agent_semantic_hook::{build_default_activation, write_activation};

use crate::test_support::{CACHE_TEST_LOCK, EnvVarGuard};

#[test]
fn cached_activation_loader_refreshes_stale_provider_command_prefix() {
    let _guard = CACHE_TEST_LOCK.lock().expect("cache test lock");
    let root = temp_project_root("activation-cache-refresh");
    let provider_v1 = root.join("provider-v1");
    let provider_v2 = root.join("provider-v2");
    write_executable(&provider_v1);
    write_executable(&provider_v2);
    let activation_path = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("hooks")
        .join("activation.json");

    write_python_provider_config(&root, "./provider-v1");
    let activation = build_default_activation(&root).expect("build initial activation");
    write_activation(&activation_path, &activation).expect("write initial activation");
    write_python_provider_config(&root, "./provider-v2");

    let _activation_path = EnvVarGuard::set(ASP_PROVIDER_ACTIVATION_PATH_ENV, &activation_path);
    let _state_home = EnvVarGuard::set("ASP_STATE_HOME", root.join(".asp-state"));
    let _ignored_cache_home = EnvVarGuard::set("PRJ_CACHE_HOME", root.join(".cache-home"));

    let snapshot = crate::activation_cache::load_provider_registry_snapshot(&root, &root, true)
        .expect("snapshot");

    let provider = snapshot
        .provider_for_language(&LanguageId::from("python"))
        .expect("python provider");
    let expected_prefix = std::fs::canonicalize(&provider_v2)
        .unwrap_or(provider_v2.clone())
        .display()
        .to_string();
    assert_eq!(
        provider.provider_command_prefix,
        vec![expected_prefix.clone()]
    );
    let rewritten = std::fs::read_to_string(&activation_path).expect("read rewritten activation");
    assert!(rewritten.contains(&expected_prefix));
    assert!(!rewritten.contains(&provider_v1.display().to_string()));
    let engine = ClientDbEngine::resolve(&root).expect("db engine");
    let db_path = engine.db_path();
    assert!(
        db_path.is_file(),
        "provider selection Turso DB should exist"
    );
    assert!(!root.join(".cache-home").exists());
    let _ = std::fs::remove_dir_all(root);
}

fn temp_project_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("asp-client-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp project root");
    root
}

fn write_python_provider_config(root: &std::path::Path, binary: &str) {
    let config_path = root.join(".agents").join("asp.toml");
    std::fs::create_dir_all(config_path.parent().expect("agent config parent"))
        .expect("create agent config parent");
    std::fs::write(
        &config_path,
        format!("[providers.python]\nenabled = true\nbinary = \"{binary}\"\n"),
    )
    .expect("write .agents/asp.toml");
}

fn write_executable(path: &std::path::Path) {
    std::fs::write(path, "#!/bin/sh\nexit 0\n").expect("write executable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .expect("executable metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("set executable permissions");
    }
}
