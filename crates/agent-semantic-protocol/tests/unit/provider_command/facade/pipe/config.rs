use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn asp_toml_search_ignore_dirs_apply_to_fast_discovery() {
    let root = temp_project_root("search-fzf-asp-toml-ignore");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::create_dir_all(root.join("generated")).expect("create generated");
    std::fs::write(root.join("src/lib.rs"), "pub fn cache_root() {}\n").expect("write source");
    std::fs::write(
        root.join("generated/lib.rs"),
        "pub fn cache_root_generated() {}\n",
    )
    .expect("write generated");
    std::fs::write(
        root.join("asp.toml"),
        "[search]\nignoreDirs = [\"generated\"]\n",
    )
    .expect("write asp.toml");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "fzf",
            "cache_root",
            "owner",
            "tests",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search fzf with asp.toml");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("src/lib.rs"), "{stdout}");
    assert!(!stdout.contains("generated/lib.rs"), "{stdout}");
    assert!(
        !marker.exists(),
        "configured fast path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_toml_language_disabled_blocks_fast_discovery() {
    let root = temp_project_root("search-fzf-language-disabled");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "pub fn cache_root() {}\n").expect("write source");
    std::fs::write(root.join("asp.toml"), "[languages.rust]\nenabled = false\n")
        .expect("write asp.toml");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "fzf",
            "cache_root",
            "owner",
            "tests",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run disabled asp rust search fzf");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("language `rust` is disabled by asp.toml"),
        "{stderr}"
    );
    assert!(
        !marker.exists(),
        "disabled language should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}
