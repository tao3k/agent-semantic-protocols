use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};
use agent_semantic_protocol::run_cli_args;
use std::time::{Duration, Instant};

const WORKSPACE_FILE_REJECTION_API_MAX: Duration = Duration::from_millis(25);

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

#[test]
fn workspace_file_is_rejected_before_provider_spawn() {
    let root = temp_project_root("search-workspace-file-rejected");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::write(root.join("build-std.ss"), "(display \"build\")\n").expect("write owner file");
    write_marker_provider(&bin_dir, "gslph", &marker);
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "search",
            "owner",
            "build-std.ss",
            "items",
            "--query",
            "builded|pended|optimization|make|clan|building",
            "--workspace",
            "build-std.ss",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp gerbil-scheme search with file workspace");

    assert!(
        !output.status.success(),
        "workspace file should fail before provider spawn"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--workspace requires a directory project root"),
        "stderr={stderr}"
    );
    assert!(
        stderr.contains("Keep the file path as the owner/selector"),
        "stderr={stderr}"
    );
    assert!(
        !marker.exists(),
        "provider must not run for invalid workspace"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn workspace_file_rejection_error_snapshot_and_perf() {
    let root = temp_project_root("search-workspace-file-rejection-api");
    let workspace_file = root.join("build-std.ss");
    std::fs::write(&workspace_file, "(display \"build\")\n").expect("write owner file");
    let workspace = workspace_file.display().to_string();

    let start = Instant::now();
    let error = run_cli_args([
        "gerbil-scheme",
        "search",
        "owner",
        "build-std.ss",
        "items",
        "--query",
        "builded|pended|optimization|make|clan|building",
        "--workspace",
        workspace.as_str(),
        "--view",
        "seeds",
    ])
    .expect_err("file-valued workspace should fail through Rust API");
    let elapsed = start.elapsed();

    assert!(
        elapsed <= WORKSPACE_FILE_REJECTION_API_MAX,
        "workspace file rejection took {elapsed:?}, expected <= {WORKSPACE_FILE_REJECTION_API_MAX:?}"
    );
    let canonical_root = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
    let snapshot = error
        .replace(&canonical_root.display().to_string(), "[ROOT]")
        .replace(&root.display().to_string(), "[ROOT]");
    insta::assert_snapshot!(
        snapshot,
        @r###"--workspace requires a directory project root, got file `[ROOT]/build-std.ss`. Keep the file path as the owner/selector and use a directory workspace, for example `asp gerbil-scheme search owner <file> items --query '<terms>' --workspace . --view seeds`."###
    );

    let _ = std::fs::remove_dir_all(root);
}
