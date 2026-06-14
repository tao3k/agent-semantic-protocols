use crate::provider_command::support::{
    asp_command, make_executable, prepend_path, provider, temp_project_root, write_activation,
    write_cache_source_fixture, write_echo_provider,
};

#[test]
fn asp_toml_provider_bin_overrides_activation_binary() {
    let root = temp_project_root("provider-bin-override-facade");
    let bin_dir = root.join(".bin");
    let override_bin = bin_dir.join("override-rs-harness");
    write_echo_provider(&bin_dir, "override-rs-harness", "override");
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
        "override args=[evidence]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_query_uses_activation_prefix_before_path_lookup() {
    let root = temp_project_root("provider-activation-prefix-facade");
    write_cache_source_fixture(&root);
    let profile_bin_dir = root.join(".profile-bin");
    let path_bin_dir = root.join(".path-bin");
    write_echo_provider(&profile_bin_dir, "rs-harness", "profile");
    std::fs::create_dir_all(&path_bin_dir).expect("create path bin dir");
    let path_provider = path_bin_dir.join("rs-harness");
    std::fs::write(
        &path_provider,
        "#!/bin/sh\nprintf 'path provider should not run\\n' >&2\nexit 42\n",
    )
    .expect("write path provider");
    make_executable(&path_provider);

    write_activation(
        &root,
        &[provider(
            "rust",
            vec![profile_bin_dir.join("rs-harness").display().to_string()],
        )],
    );

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("PATH", prepend_path(&path_bin_dir))
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
        "profile args=[query][src/lib.rs]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_query_guide_routes_to_activation_prefix() {
    let root = temp_project_root("provider-query-guide-prefix-facade");
    let profile_bin_dir = root.join(".profile-bin");
    let path_bin_dir = root.join(".path-bin");
    write_echo_provider(&profile_bin_dir, "rs-harness", "profile");
    std::fs::create_dir_all(&path_bin_dir).expect("create path bin dir");
    let path_provider = path_bin_dir.join("rs-harness");
    std::fs::write(
        &path_provider,
        "#!/bin/sh\nprintf 'path provider should not run\\n' >&2\nexit 42\n",
    )
    .expect("write path provider");
    make_executable(&path_provider);

    write_activation(
        &root,
        &[provider(
            "rust",
            vec![profile_bin_dir.join("rs-harness").display().to_string()],
        )],
    );

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("PATH", prepend_path(&path_bin_dir))
        .args(["rust", "query", "guide", "."])
        .output()
        .expect("run asp rust query guide");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "profile args=[query][guide]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_search_guide_routes_to_activation_prefix() {
    let root = temp_project_root("provider-search-guide-prefix-facade");
    let profile_bin_dir = root.join(".profile-bin");
    let path_bin_dir = root.join(".path-bin");
    write_echo_provider(&profile_bin_dir, "rs-harness", "profile");
    std::fs::create_dir_all(&path_bin_dir).expect("create path bin dir");
    let path_provider = path_bin_dir.join("rs-harness");
    std::fs::write(
        &path_provider,
        "#!/bin/sh\nprintf 'path provider should not run\\n' >&2\nexit 42\n",
    )
    .expect("write path provider");
    make_executable(&path_provider);

    write_activation(
        &root,
        &[provider(
            "rust",
            vec![profile_bin_dir.join("rs-harness").display().to_string()],
        )],
    );

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("PATH", prepend_path(&path_bin_dir))
        .args(["rust", "search", "guide", "."])
        .output()
        .expect("run asp rust search guide");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "profile args=[search][guide]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}
