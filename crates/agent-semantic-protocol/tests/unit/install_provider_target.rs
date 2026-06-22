use super::{resolve_provider_binary_install_target, resolve_provider_binary_invocation};

#[test]
fn provider_install_target_prefers_asp_toml_absolute_bin() {
    let root = std::env::temp_dir().join("asp-install-target-config-root");
    let home = std::env::temp_dir().join("asp-install-target-config-home");
    let path_dir = std::env::temp_dir().join("asp-install-target-config-path");
    let configured = root.join("tools/rs-harness-from-config");
    let target = resolve_provider_binary_install_target(
        Some(configured.to_str().expect("utf8 path")),
        None,
        "rust",
        "rs-harness",
        &root,
        Some(&home),
        &[path_dir],
    )
    .expect("install target");

    assert_eq!(target.path, configured);
    assert_eq!(target.source, "asp.toml");
}

#[test]
fn provider_install_target_uses_home_local_bin_before_path() {
    let root = std::env::temp_dir().join("asp-install-target-home-root");
    let home = std::env::temp_dir().join("asp-install-target-home-home");
    let path_dir = std::env::temp_dir().join("asp-install-target-home-path");
    let target = resolve_provider_binary_install_target(
        None,
        None,
        "rust",
        "rs-harness",
        &root,
        Some(&home),
        &[path_dir],
    )
    .expect("install target");

    assert_eq!(target.path, home.join(".local/bin/rs-harness"));
    assert_eq!(target.source, "home-local-bin");
}

#[test]
fn provider_install_target_uses_semantic_agent_bin_dir_before_home_and_path() {
    let root = std::env::temp_dir().join("asp-install-target-env-root");
    let env_bin = std::env::temp_dir().join("asp-install-target-env-bin");
    let home = std::env::temp_dir().join("asp-install-target-env-home");
    let path_dir = std::env::temp_dir().join("asp-install-target-env-path");
    std::fs::create_dir_all(&path_dir).expect("create path dir");
    std::fs::write(path_dir.join("rs-harness"), "").expect("write path provider");
    let target = resolve_provider_binary_install_target(
        None,
        Some(&env_bin),
        "rust",
        "rs-harness",
        &root,
        Some(&home),
        &[path_dir],
    )
    .expect("install target");

    assert_eq!(target.path, env_bin.join("rs-harness"));
    assert_eq!(target.source, "semantic-agent-bin-dir");
}

#[test]
fn provider_install_target_falls_back_to_existing_path_without_home() {
    let root = std::env::temp_dir().join("asp-install-target-path-root");
    let path_dir = std::env::temp_dir().join("asp-install-target-path-bin");
    std::fs::create_dir_all(&path_dir).expect("create path dir");
    let existing = path_dir.join("rs-harness");
    std::fs::write(&existing, "").expect("write existing provider");
    let target = resolve_provider_binary_install_target(
        None,
        None,
        "rust",
        "rs-harness",
        &root,
        None,
        &[path_dir],
    )
    .expect("install target");

    assert_eq!(target.path, existing);
    assert_eq!(target.source, "path-existing");
}

#[test]
fn provider_invocation_prefers_asp_toml_relative_bin() {
    let root = std::env::temp_dir().join("asp-invocation-target-config-root");
    let home = std::env::temp_dir().join("asp-invocation-target-config-home");
    let path_dir = std::env::temp_dir().join("asp-invocation-target-config-path");
    let invocation = resolve_provider_binary_invocation(
        Some("tools/rs-harness-config"),
        "rs-harness",
        &root,
        Some(&home),
        &[path_dir],
    )
    .expect("invocation")
    .expect("configured invocation");

    assert_eq!(
        invocation.command,
        root.join("tools/rs-harness-config").to_string_lossy()
    );
    assert_eq!(invocation.source, "asp.toml");
}

#[test]
fn provider_invocation_prefers_project_bin_before_path_and_home() {
    let root = std::env::temp_dir().join("asp-invocation-target-project-root");
    let project_bin = root.join(".bin");
    let home = std::env::temp_dir().join("asp-invocation-target-project-home");
    let home_bin = home.join(".local/bin");
    let path_dir = std::env::temp_dir().join("asp-invocation-target-project-path");
    std::fs::create_dir_all(&project_bin).expect("create project bin dir");
    std::fs::write(project_bin.join("rs-harness"), "").expect("write project provider");
    std::fs::create_dir_all(&home_bin).expect("create home bin dir");
    std::fs::write(home_bin.join("rs-harness"), "").expect("write home provider");
    std::fs::create_dir_all(&path_dir).expect("create path dir");
    std::fs::write(path_dir.join("rs-harness"), "").expect("write path provider");
    let invocation =
        resolve_provider_binary_invocation(None, "rs-harness", &root, Some(&home), &[path_dir])
            .expect("invocation")
            .expect("project invocation");

    assert_eq!(
        invocation.command,
        root.join(".bin/rs-harness").to_string_lossy()
    );
    assert_eq!(invocation.source, "project-bin");
}

#[test]
fn provider_invocation_prefers_path_before_home_local_bin() {
    let root = std::env::temp_dir().join("asp-invocation-target-path-priority-root");
    let home = std::env::temp_dir().join("asp-invocation-target-path-priority-home");
    let home_bin = home.join(".local/bin");
    let path_dir = std::env::temp_dir().join("asp-invocation-target-path-priority-path");
    std::fs::create_dir_all(&home_bin).expect("create home bin dir");
    std::fs::write(home_bin.join("rs-harness"), "").expect("write home provider");
    std::fs::create_dir_all(&path_dir).expect("create path dir");
    let existing = path_dir.join("rs-harness");
    std::fs::write(&existing, "").expect("write path provider");
    let invocation =
        resolve_provider_binary_invocation(None, "rs-harness", &root, Some(&home), &[path_dir])
            .expect("invocation")
            .expect("path invocation");

    assert_eq!(invocation.command, existing.to_string_lossy());
    assert_eq!(invocation.source, "path-existing");
}

#[test]
fn provider_invocation_prefers_project_bin_for_bare_asp_toml_bin() {
    let root = std::env::temp_dir().join("asp-invocation-target-bare-config-root");
    let project_bin = root.join(".bin");
    let home = std::env::temp_dir().join("asp-invocation-target-bare-config-home");
    let home_bin = home.join(".local/bin");
    let path_dir = std::env::temp_dir().join("asp-invocation-target-bare-config-path");
    std::fs::create_dir_all(&project_bin).expect("create project bin dir");
    std::fs::write(project_bin.join("rs-harness"), "").expect("write project provider");
    std::fs::create_dir_all(&home_bin).expect("create home bin dir");
    std::fs::write(home_bin.join("rs-harness"), "").expect("write home provider");
    std::fs::create_dir_all(&path_dir).expect("create path dir");
    std::fs::write(path_dir.join("rs-harness"), "").expect("write path provider");
    let invocation = resolve_provider_binary_invocation(
        Some("rs-harness"),
        "rs-harness",
        &root,
        Some(&home),
        &[path_dir],
    )
    .expect("invocation")
    .expect("configured invocation");

    assert_eq!(
        invocation.command,
        root.join(".bin/rs-harness").to_string_lossy()
    );
    assert_eq!(invocation.source, "asp.toml");
}

#[test]
fn provider_invocation_falls_back_to_path_without_home_bin() {
    let root = std::env::temp_dir().join("asp-invocation-target-path-root");
    let home = std::env::temp_dir().join("asp-invocation-target-path-home");
    let path_dir = std::env::temp_dir().join("asp-invocation-target-path-bin");
    std::fs::create_dir_all(&path_dir).expect("create path dir");
    let existing = path_dir.join("rs-harness");
    std::fs::write(&existing, "").expect("write path provider");
    let invocation =
        resolve_provider_binary_invocation(None, "rs-harness", &root, Some(&home), &[path_dir])
            .expect("invocation")
            .expect("path invocation");

    assert_eq!(invocation.command, existing.to_string_lossy());
    assert_eq!(invocation.source, "path-existing");
}
