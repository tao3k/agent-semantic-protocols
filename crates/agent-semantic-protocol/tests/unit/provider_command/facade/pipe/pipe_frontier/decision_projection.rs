use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_stdout_stderr_provider,
};

#[test]
fn search_pipe_seeds_omits_graph_projection_from_default_stdout() {
    let root = temp_project_root("search-pipe-owner-only-decision-projection");
    let bin_dir = root.join(".bin");
    let package_root = root.join("languages/rust-harness");
    std::fs::create_dir_all(package_root.join("src")).expect("create package src");
    std::fs::write(
        package_root.join("src/lib.rs"),
        "pub mod alpha;\npub mod beta;\n",
    )
    .expect("write lib");
    std::fs::write(package_root.join("src/alpha.rs"), "pub fn one() {}\n").expect("write alpha");
    std::fs::write(package_root.join("src/beta.rs"), "pub fn two() {}\n").expect("write beta");
    write_stdout_stderr_provider(
        &bin_dir,
        "rs-harness",
        r#"{"nodes":[{"id":"owner:src/alpha.rs","kind":"owner","role":"path","value":"src/alpha.rs","action":"owner","path":"src/alpha.rs","ownerPath":"src/alpha.rs","matchText":"src/alpha.rs"},{"id":"owner:src/beta.rs","kind":"owner","role":"path","value":"src/beta.rs","action":"owner","path":"src/beta.rs","ownerPath":"src/beta.rs","matchText":"src/beta.rs"}],"edges":[{"source":"owner:src/alpha.rs","target":"owner:src/beta.rs","relation":"adjacent"}]}"#,
        "",
    );
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "semantic locator route",
            "--view",
            "seeds",
            "languages/rust-harness",
        ])
        .output()
        .expect("run asp rust search pipe seeds");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(!stdout.contains("[graph-frontier]"), "{stdout}");
    assert!(!stdout.contains("evidenceNodes="), "{stdout}");
    assert!(!stdout.contains("evidenceEdges="), "{stdout}");
    assert!(!stdout.contains("rankedEvidence="), "{stdout}");
    assert!(!stdout.contains("evidenceFrontier="), "{stdout}");
    assert!(stdout.contains("nextCommand="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}
