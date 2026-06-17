use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn search_pipe_commands_view_points_to_search_suggest() {
    let root = temp_project_root("search-pipe-commands-facade");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "HookDecision",
            "--workspace",
            ".",
            "--view",
            "commands",
        ])
        .output()
        .expect("run asp rust search pipe commands");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("search pipe --view commands moved to search suggest --view commands"),
        "{stderr}"
    );
    assert!(
        !marker.exists(),
        "commands migration error should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}
