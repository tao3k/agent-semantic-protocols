use crate::provider_command::support;
#[test]
fn search_pipe_seeds_promotes_matching_dependency_route() {
    let root = support::temp_project_root("search-pipe-seeds-dependency-action");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"dep-action-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nserde = \"1\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(
        root.join("src/lib.rs"),
        "use serde::Serialize;\npub struct Receipt;\n",
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
            "pipe",
            "serde|dependency",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe seeds");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    support::assert_compact_search_action_contract(&stdout);
    assert!(
        stdout.contains("nextCommand=asp rust search deps serde --workspace . --view seeds"),
        "{stdout}"
    );
    assert!(!stdout.contains("actionFrontier="), "{stdout}");
    assert!(!stdout.contains("recommendedNext="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_does_not_promote_dependency_route_from_natural_tree_word() {
    let root = support::temp_project_root("search-pipe-seeds-natural-tree-not-dependency");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"tree-word-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\ntree-sitter = \"0.26\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn reasoning_tree_frontier() {}\n",
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
            "pipe",
            "reasoning tree|seed action frontier",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe seeds");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("search-deps(dependency=tree-sitter"),
        "{stdout}"
    );
    assert!(!stdout.contains("recommendedNext="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_does_not_promote_dependency_route_from_meta_audit_query() {
    let root = support::temp_project_root("search-pipe-seeds-meta-audit-not-dependency");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"meta-audit-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\ntree-sitter = \"0.26\"\ntokio = \"1\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn reasoning_tree_frontier() {}\n",
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
            "pipe",
            "audit evidence state reasoning tree|expected tests conclusions next plan dependency seed line selector",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe seeds");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("search-deps(dependency="), "{stdout}");
    assert!(!stdout.contains("recommendedNext="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_does_not_promote_dependency_route_from_negative_meta_query_with_literal() {
    let root = support::temp_project_root("search-pipe-seeds-meta-literal-not-dependency");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"meta-literal-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\ntree-sitter = \"0.26\"\ntokio = \"1\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn reasoning_tree_frontier() {}\n",
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
            "pipe",
            "audit meta query dependency reasoning tree|should not route to tree-sitter search-deps",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe seeds");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("search-deps(dependency="), "{stdout}");
    assert!(!stdout.contains("recommendedNext="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}
