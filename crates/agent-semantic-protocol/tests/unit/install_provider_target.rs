use super::{
    resolve_provider_binary_install_target, resolve_provider_binary_install_target_with_bin_dir,
    resolve_provider_binary_invocation,
};

#[test]
fn provider_install_target_uses_semantic_agent_bin_dir_when_set() {
    let home = std::env::temp_dir().join("asp-install-target-home-with-env");
    let bin_dir = std::env::temp_dir().join("asp-install-target-env-bin");

    let target = resolve_provider_binary_install_target_with_bin_dir(
        "rust",
        "rs-harness",
        Some(&home),
        Some(&bin_dir),
    )
    .expect("install target");

    assert_eq!(target.path, bin_dir.join("rs-harness"));
    assert_eq!(target.source, "semantic-agent-bin-dir");
}

#[test]
fn provider_install_target_uses_home_local_bin_by_default() {
    let home = std::env::temp_dir().join("asp-install-target-home-only-home");

    let target = resolve_provider_binary_install_target("rust", "rs-harness", Some(&home))
        .expect("install target");

    assert_eq!(target.path, home.join(".local/bin/rs-harness"));
    assert_eq!(target.source, "home-local-bin");
}

#[test]
fn provider_install_target_requires_home() {
    let error = resolve_provider_binary_install_target("rust", "rs-harness", None)
        .expect_err("missing HOME should fail");

    assert!(error.contains("$HOME/.local/bin/rs-harness"), "{error}");
    assert!(error.contains("HOME is not set"), "{error}");
}

#[test]
fn provider_invocation_uses_home_local_bin_only() {
    let home = std::env::temp_dir().join("asp-invocation-home-only-home");
    let home_bin = home.join(".local/bin");
    std::fs::create_dir_all(&home_bin).expect("create home bin dir");
    std::fs::write(home_bin.join("rs-harness"), "").expect("write home provider");

    let invocation =
        resolve_provider_binary_invocation("rust", "rs-harness", Some(&home)).expect("invocation");

    assert_eq!(
        invocation.command,
        home.join(".local/bin/rs-harness").to_string_lossy()
    );
    assert_eq!(invocation.source, "home-local-bin");
}

#[test]
fn provider_invocation_rejects_missing_home_local_bin_without_fallback() {
    let home = std::env::temp_dir().join("asp-invocation-missing-home-home");

    let error = resolve_provider_binary_invocation("rust", "rs-harness", Some(&home))
        .expect_err("missing home-local provider should fail");

    assert!(
        error.contains("provider binary `rs-harness` for language `rust` must be installed at"),
        "{error}"
    );
    assert!(error.contains("asp install language rust"), "{error}");
}
