use super::{resolve_provider_binary_install_target, resolve_provider_binary_invocation};

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
fn provider_invocation_uses_home_local_even_when_other_bin_dir_exists() {
    let home = std::env::temp_dir().join("asp-invocation-home-with-extra-bin");
    let extra_bin = std::env::temp_dir().join("asp-invocation-extra-bin");
    let home_bin = home.join(".local/bin");
    std::fs::create_dir_all(&home_bin).expect("create home bin dir");
    std::fs::create_dir_all(&extra_bin).expect("create extra bin dir");
    std::fs::write(home_bin.join("rs-harness"), "").expect("write home provider");
    std::fs::write(extra_bin.join("rs-harness"), "").expect("write extra provider");

    let invocation =
        resolve_provider_binary_invocation("rust", "rs-harness", Some(&home)).expect("invocation");

    assert_eq!(
        invocation.command,
        home.join(".local/bin/rs-harness").to_string_lossy()
    );
    assert_eq!(invocation.source, "home-local-bin");
}

#[test]
fn provider_invocation_uses_home_local_bin_by_default() {
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

    assert!(error.contains("state=provider-binary-missing"), "{error}");
    assert!(error.contains("language=rust"), "{error}");
    assert!(error.contains("binary=rs-harness"), "{error}");
    assert!(error.contains("installMode=locked-release"), "{error}");
    assert!(
        error.contains("nextCommand=asp install language rust"),
        "{error}"
    );
}
