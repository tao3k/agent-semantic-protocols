use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_echo_provider,
};

#[test]
fn rust_search_facade_fans_out_multiple_trailing_scope_roots() {
    let root = temp_project_root("rust-search-facade-multi-scope");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("crates/agent-semantic-hook")).expect("create hook scope");
    std::fs::create_dir_all(root.join("crates/agent-semantic-protocol"))
        .expect("create protocol scope");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "rust",
            "search",
            "fzf",
            "--query-set",
            "reasonKind",
            "--query-set",
            "RawBroadSearch",
            "owner",
            "tests",
            "--view",
            "seeds",
            "crates/agent-semantic-hook",
            "crates/agent-semantic-protocol",
        ])
        .output()
        .expect("run asp rust search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        concat!(
            "rs args=[search][fzf][--query-set][reasonKind][--query-set][RawBroadSearch][owner][tests][--view][seeds][crates/agent-semantic-hook]\n",
            "rs args=[search][fzf][--query-set][reasonKind][--query-set][RawBroadSearch][owner][tests][--view][seeds][crates/agent-semantic-protocol]\n",
        )
    );
    let _ = std::fs::remove_dir_all(root);
}
