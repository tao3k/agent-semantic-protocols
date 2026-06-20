use super::resolve_provider_binary_install_target;

#[test]
fn provider_install_target_prefers_asp_toml_absolute_bin() {
    let root = std::env::temp_dir().join("asp-install-target-config-root");
    let home = std::env::temp_dir().join("asp-install-target-config-home");
    let path_dir = std::env::temp_dir().join("asp-install-target-config-path");
    let configured = root.join("tools/rs-harness-from-config");
    let target = resolve_provider_binary_install_target(
        Some(configured.to_str().expect("utf8 path")),
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
fn provider_install_target_falls_back_to_existing_path_without_home() {
    let root = std::env::temp_dir().join("asp-install-target-path-root");
    let path_dir = std::env::temp_dir().join("asp-install-target-path-bin");
    std::fs::create_dir_all(&path_dir).expect("create path dir");
    let existing = path_dir.join("rs-harness");
    std::fs::write(&existing, "").expect("write existing provider");
    let target = resolve_provider_binary_install_target(
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
