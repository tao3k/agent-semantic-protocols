use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_echo_provider,
    write_stdout_stderr_provider,
};

#[test]
fn root_search_facade_infers_language_from_owner_path() {
    let root = temp_project_root("root-query-infer-language");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "fn demo() {}\n").expect("write rust source");
    write_stdout_stderr_provider(
        &bin_dir,
        "rs-harness",
        "[search-owner] q=src/lib.rs pkg=. selector=items alg=item-frontier\n\
O=owner:path(src/lib.rs)!owner;I=item:symbol(demo)!syntax\n\
O>{I:contains}\n\
rank=I,O frontier=I.syntax\n",
        "",
    );
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "search",
            "owner",
            "src/lib.rs",
            "items",
            "--query",
            "demo",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp search with inferred language");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[search-owner]"), "{stdout}");
    assert!(stdout.contains("item:symbol(demo)"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn root_search_facade_consumes_workspace_for_explicit_language() {
    let root = temp_project_root("root-query-workspace-default");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "fn demo() {}\n").expect("write rust source");
    write_stdout_stderr_provider(
        &bin_dir,
        "rs-harness",
        "[search-owner] q=src/lib.rs pkg=. selector=items alg=item-frontier\n\
O=owner:path(src/lib.rs)!owner;I=item:symbol(demo)!syntax\n\
O>{I:contains}\n\
rank=I,O frontier=I.syntax\n",
        "",
    );
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "search",
            "--language",
            "rust",
            "owner",
            "src/lib.rs",
            "items",
            "--query",
            "demo",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp search with explicit language and workspace");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[search-owner]"), "{stdout}");
    assert!(stdout.contains("item:symbol(demo)"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn root_search_facade_owns_workspace_tree_sitter_discovery() {
    let root = temp_project_root("root-query-view-seeds");
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
            "--treesitter-query",
            "(function_item name: (identifier) @function.name)",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp search Tree-sitter discovery");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[search-treesitter]"), "{stdout}");
    assert!(
        stdout.contains("mode: structural Tree-sitter reasoning search"),
        "{stdout}"
    );
    assert!(!stdout.contains("rs args="), "{stdout}");
    assert!(!stdout.contains("--view"), "{stdout}");
    assert!(!stdout.contains("seeds"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}
