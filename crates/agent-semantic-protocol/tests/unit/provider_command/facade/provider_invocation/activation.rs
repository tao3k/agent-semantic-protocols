use crate::provider_command::support::{
    asp_command, home_local_bin, provider, temp_project_root, write_activation,
    write_cache_source_fixture, write_echo_provider,
};

#[test]
fn asp_toml_provider_bin_does_not_override_home_local_binary() {
    let root = temp_project_root("provider-bin-home-only-facade");
    let home_bin = home_local_bin(&root);
    let override_bin_dir = root.join(".bin");
    let override_bin = override_bin_dir.join("override-rs-harness");
    write_echo_provider(&home_bin, "rs-harness", "home");
    write_echo_provider(&override_bin_dir, "override-rs-harness", "override");
    std::fs::write(
        root.join("asp.toml"),
        format!("[languages.rust]\nbin = \"{}\"\n", override_bin.display()),
    )
    .expect("write asp.toml");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env_remove("PATH")
        .args(["rust", "evidence", "."])
        .output()
        .expect("run asp rust evidence with provider bin override");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "home args=[evidence]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_prefers_home_local_provider_before_activation_prefix() {
    let root = temp_project_root("provider-home-wrapper-facade");
    let home_bin = home_local_bin(&root);
    let profile_bin_dir = root.join(".profile-bin");
    write_echo_provider(&home_bin, "rs-harness", "home");
    write_echo_provider(&profile_bin_dir, "rs-harness", "profile");

    write_activation(
        &root,
        &[provider(
            "rust",
            vec![profile_bin_dir.join("rs-harness").display().to_string()],
        )],
    );

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env_remove("PATH")
        .args(["rust", "evidence", "."])
        .output()
        .expect("run asp rust evidence");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "home args=[evidence]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_query_uses_home_local_binary_before_activation_prefix() {
    let root = temp_project_root("provider-query-home-only-facade");
    write_cache_source_fixture(&root);
    let home_bin = home_local_bin(&root);
    let profile_bin_dir = root.join(".profile-bin");
    write_echo_provider(&home_bin, "rs-harness", "home");
    write_echo_provider(&profile_bin_dir, "rs-harness", "profile");

    write_activation(
        &root,
        &[provider(
            "rust",
            vec![profile_bin_dir.join("rs-harness").display().to_string()],
        )],
    );

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env_remove("PATH")
        .args(["rust", "query", "src/lib.rs", "."])
        .output()
        .expect("run asp rust query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "home args=[query][src/lib.rs]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_guide_uses_home_local_binary_before_activation_prefix() {
    let root = temp_project_root("provider-guide-home-only-facade");
    let home_bin = home_local_bin(&root);
    let profile_bin_dir = root.join(".profile-bin");
    write_echo_provider(&home_bin, "rs-harness", "home");
    write_echo_provider(&profile_bin_dir, "rs-harness", "profile");

    write_activation(
        &root,
        &[provider(
            "rust",
            vec![profile_bin_dir.join("rs-harness").display().to_string()],
        )],
    );

    for args in [
        ["rust", "query", "guide", "."],
        ["rust", "search", "guide", "."],
    ] {
        let output = asp_command(&root)
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .env_remove("PATH")
            .args(args)
            .output()
            .expect("run asp rust guide");

        assert!(
            output.status.success(),
            "args={args:?} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert!(stdout.starts_with("home args="), "{stdout}");
    }
    let _ = std::fs::remove_dir_all(root);
}
