use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn low_cohesion_query_pack_precedes_global_fd_discovery() {
    let root = temp_project_root("search-pipe-low-cohesion-query-pack-before-fd");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    for (package, body) in [
        ("alpha", "pub fn alpha_runtime_probe() {}\n"),
        ("beta", "pub fn beta_queue_probe() {}\n"),
        ("gamma", "pub fn gamma_graph_probe() {}\n"),
        ("delta", "pub fn delta_store_probe() {}\n"),
    ] {
        let src_dir = root.join(package).join("src");
        std::fs::create_dir_all(&src_dir).expect("create source dir");
        std::fs::write(src_dir.join("lib.rs"), body).expect("write source");
    }
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "alpha beta gamma delta runtime queue graph store",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search pipe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("packageCohesion=low"), "{stdout}");
    assert!(
        stdout.contains("riskFactors=flat-query,owner-drift"),
        "{stdout}"
    );
    assert!(stdout.contains("nextCommand=asp rg -query"), "{stdout}");
    assert!(!stdout.contains("actionFrontier="), "{stdout}");
    assert!(!stdout.contains("recommendedNext="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn low_cohesion_query_pack_materializes_dominant_owner_package_scope() {
    let root = temp_project_root("search-pipe-low-cohesion-query-pack-scoped-command");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    for (path, body) in [
        (
            "crates/agent-semantic-protocol/src/command/search_pipe_actions.rs",
            "pub fn low_cohesion_rg_query_set_command_scope_package_graph_turbo_action_ranking() {}\n",
        ),
        (
            "crates/agent-semantic-protocol/src/command/search_pipe_graph_turbo.rs",
            "pub fn graph_turbo_action_ranking_package_scope_query_set() {}\n",
        ),
        (
            "crates/agent-semantic-protocol/src/command/search_pipe_quality.rs",
            "pub fn low_cohesion_package_scope_finder_drift() {}\n",
        ),
        (
            "crates/agent-semantic-client/src/cache_cli.rs",
            "pub fn command_scope_finder_drift_client_probe() {}\n",
        ),
        (
            "languages/orgize/src/syntax/paragraph.rs",
            "pub fn low_query_package_drift_probe() {}\n",
        ),
    ] {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create source dir");
        std::fs::write(path, body).expect("write source");
    }
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "low cohesion rg query set command scope package graph turbo action ranking finder drift",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search pipe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("packageCohesion=low"), "{stdout}");
    assert!(
        stdout.contains("nextCommand=asp rg -query")
            && stdout.contains("--workspace crates/agent-semantic-protocol"),
        "{stdout}"
    );
    assert!(!stdout.contains("actionFrontier="), "{stdout}");
    assert!(!stdout.contains("recommendedNext="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}
