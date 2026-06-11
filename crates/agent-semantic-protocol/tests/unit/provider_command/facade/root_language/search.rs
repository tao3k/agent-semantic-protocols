use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_echo_provider,
};

#[test]
fn root_search_facade_routes_explicit_language_to_provider() {
    let root = temp_project_root("root-search-explicit-language");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "search",
            "--language",
            "rust",
            "prime",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp search explicit language");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "rs args=[search][prime][--view][seeds]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn root_search_facade_infers_language_from_single_project_marker() {
    let root = temp_project_root("root-search-single-marker");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='demo'\nversion='0.1.0'\n",
    )
    .expect("write cargo manifest");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args(["search", "prime", "--view", "seeds", "."])
        .output()
        .expect("run asp search inferred language");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "rs args=[search][prime][--view][seeds]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn root_search_facade_requires_language_when_project_markers_are_ambiguous() {
    let root = temp_project_root("root-search-ambiguous-language");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='demo'\nversion='0.1.0'\n",
    )
    .expect("write cargo manifest");
    std::fs::write(
        root.join("pyproject.toml"),
        "[project]\nname='demo'\nversion='0.1.0'\n",
    )
    .expect("write pyproject");

    let output = asp_command(&root)
        .args(["search", "prime", "--view", "seeds", "."])
        .output()
        .expect("run asp search ambiguous language");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("asp search requires --language"));
    assert!(stderr.contains("asp <language> search"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn root_search_facade_rejects_unsupported_explicit_language_with_finder_recovery() {
    let root = temp_project_root("root-search-unsupported-language");
    write_activation(
        &root,
        &[
            provider("gerbil-scheme", Vec::new()),
            provider("rust", Vec::new()),
            provider("typescript", Vec::new()),
        ],
    );

    let output = asp_command(&root)
        .args([
            "search",
            "--language",
            "scheme",
            "prime",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp search unsupported language");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("unsupported ASP language facade `scheme`"),
        "{stderr}"
    );
    assert!(
        stderr.contains("Active language facades: gerbil-scheme|rust|typescript."),
        "{stderr}"
    );
    assert!(stderr.contains("asp providers"), "{stderr}");
    assert!(stderr.contains("asp rg -query"), "{stderr}");
    assert!(
        stderr.contains("Do not switch to an unrelated active facade"),
        "{stderr}"
    );
    assert!(!stderr.contains("asp typescript search prime"), "{stderr}");
    assert!(!stderr.contains("Suggested matching facade"), "{stderr}");
    assert!(!stderr.contains("asp gerbil-scheme search"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
}
