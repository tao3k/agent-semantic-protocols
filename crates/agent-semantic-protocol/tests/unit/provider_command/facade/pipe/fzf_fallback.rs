use crate::provider_command::support::{
    asp_command, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn fzf_fallback_collector_matches_multiple_terms_without_native_finder() {
    let root = temp_project_root("search-fzf-fallback-multi-term");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/cache_root.rs"),
        "pub fn providerneedle() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    std::fs::write(
        root.join("src/providerneedle.txt"),
        "cache_root providerneedle\n",
    )
    .expect("write ignored text source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "fzf",
            "cache_root|providerneedle",
            "owner",
            "items",
            "tests",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search fzf without native finder");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.starts_with("[graph-frontier] profile=owner-query"),
        "{stdout}"
    );
    assert!(stdout.contains("item:symbol(cache_root)"), "{stdout}");
    assert!(stdout.contains("item:symbol(providerneedle)"), "{stdout}");
    assert!(!stdout.contains(".txt"), "{stdout}");
    assert!(
        !marker.exists(),
        "fallback fzf seeds should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}
