use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_echo_provider,
};

#[test]
fn root_query_facade_infers_language_from_owner_path() {
    let root = temp_project_root("root-query-infer-language");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "fn demo() {}\n").expect("write rust source");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "query",
            "src/lib.rs",
            "--query",
            "demo",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp query inferred language");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "fn demo() {}\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn root_query_facade_consumes_workspace_for_default_query() {
    let root = temp_project_root("root-query-workspace-default");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "fn demo() {}\n").expect("write rust source");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args(["query", "src/lib.rs", "--term", "demo", "--workspace", "."])
        .output()
        .expect("run asp query inferred language with workspace");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[search-owner] q=src/lib.rs"), "{stdout}");
    assert!(stdout.contains("|item name=demo kind=function"), "{stdout}");
    assert!(
        stdout.contains("|query itemQuery=demo status=hit"),
        "{stdout}"
    );
    assert!(!stdout.contains("rs args="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn root_query_facade_strips_seed_view_before_provider_dispatch() {
    let root = temp_project_root("root-query-view-seeds");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "query",
            "--language",
            "rust",
            "--treesitter-query",
            "(function_item name: (identifier) @function.name)",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp query view seeds");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("rs args=[query][--treesitter-query]"),
        "{stdout}"
    );
    assert!(!stdout.contains("--view"), "{stdout}");
    assert!(!stdout.contains("seeds"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}
