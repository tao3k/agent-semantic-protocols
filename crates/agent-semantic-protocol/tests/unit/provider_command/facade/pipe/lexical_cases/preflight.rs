use crate::provider_command::support;

#[test]
fn lexical_missing_view_value_reports_seeds_contract_before_provider_spawn() {
    let root = support::temp_project_root("search-lexical-missing-view-value");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "cache_root",
            "CacheRoot",
            "owner",
            "items",
            "--workspace",
            ".",
            "--view",
        ])
        .output()
        .expect("run asp rust search lexical missing view value");

    assert!(!output.status.success(), "search unexpectedly succeeded");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("search lexical --view requires seeds"),
        "{stderr}"
    );
    assert!(
        !marker.exists(),
        "missing-view lexical rejection should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn lexical_rejects_owner_dependency_surface_combination_without_provider_spawn() {
    let root = support::temp_project_root("search-lexical-owner-deps-rejected");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "gxpkg|package",
            "owner",
            "deps",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search lexical owner deps");

    assert!(!output.status.success(), "search unexpectedly succeeded");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("search lexical does not support combining deps with owner/items"),
        "{stderr}"
    );
    assert!(
        !marker.exists(),
        "rejected lexical surface combination should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn lexical_seeds_is_asp_owned_for_cheap_discovery() {
    let root = support::temp_project_root("search-lexical-fast-facade");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn cache_root() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "cache_root",
            "unrelated",
            "owner",
            "items",
            "tests",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search lexical");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_builtin_graph_route(&stdout, "cache_root");
    assert!(
        !marker.exists(),
        "search lexical seeds should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn lexical_rejects_single_seed_before_provider_spawn() {
    let root = support::temp_project_root("search-lexical-single-seed-rejected");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "cache_root",
            "owner",
            "items",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search lexical single seed");

    assert!(!output.status.success(), "search unexpectedly succeeded");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("query-bundle-required"), "{stderr}");
    assert!(
        !marker.exists(),
        "single-seed lexical rejection should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}
