use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn search_pipe_plan_preserves_search_scope_in_primary_command() {
    let root = temp_project_root("search-pipe-package-root-commands");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    let package_root = root.join("languages/rust-harness");
    std::fs::create_dir_all(package_root.join("src")).expect("create package src");
    std::fs::write(
        package_root.join("src/lib.rs"),
        "pub struct HookDecision;\npub struct ClientReceipt;\n",
    )
    .expect("write package source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "HookDecision ClientReceipt",
            "--view",
            "seeds",
            "languages/rust-harness",
        ])
        .output()
        .expect("run asp rust search pipe for package root");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(!stdout.contains("context=>"), "{stdout}");
    assert!(!stdout.contains("pipe=>asp rust search pipe"), "{stdout}");
    assert!(
        stdout.contains("nextCommand=asp rust query --selector languages/rust-harness/src/lib.rs:")
            && stdout.contains(" --workspace . --code"),
        "{stdout}"
    );
    assert!(
        stdout.contains("S1=>asp rust query --selector languages/rust-harness/src/lib.rs:")
            && stdout.contains(" --workspace . --code"),
        "{stdout}"
    );
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}
