use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn fzf_accepts_workspace_and_trailing_scope_path() {
    let root = temp_project_root("search-fzf-workspace-scope");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    let scoped_dir = root.join("packages/python/asp_graph_turbo/src/asp_graph_turbo");
    std::fs::create_dir_all(&scoped_dir).expect("create scoped dir");
    std::fs::create_dir_all(root.join("tests/unit")).expect("create tests dir");
    std::fs::write(
        scoped_dir.join("calibration.py"),
        "def calibration():\n    return 1\n",
    )
    .expect("write scoped source");
    std::fs::write(
        root.join("tests/unit/noise.py"),
        "def calibration():\n    return 2\n",
    )
    .expect("write unscoped source");
    write_marker_provider(&bin_dir, "py-harness", &marker);
    write_activation(&root, &[provider("python", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "python",
            "search",
            "fzf",
            "calibration",
            "--workspace",
            ".",
            "--view",
            "seeds",
            "packages/python/asp_graph_turbo/src/asp_graph_turbo",
        ])
        .output()
        .expect("run asp python search fzf with workspace scope");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[graph-frontier]"), "{stdout}");
    assert!(
        stdout.contains("packages/python/asp_graph_turbo/src/asp_graph_turbo/calibration.py"),
        "{stdout}"
    );
    assert!(!stdout.contains("tests/unit/noise.py"), "{stdout}");
    assert!(!marker.exists(), "search fzf should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}
