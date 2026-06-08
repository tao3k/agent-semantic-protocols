use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn search_pipe_rejects_removed_pipeline_option_without_provider_spawn() {
    let root = temp_project_root("search-pipe-rejects-pipe-option");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    let removed_option = format!("--{}", "pipe");

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(vec![
            "rust".to_string(),
            "search".to_string(),
            "pipe".to_string(),
            "HookDecision".to_string(),
            removed_option.clone(),
            "items,tests".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
            ".".to_string(),
        ])
        .output()
        .expect("run asp rust search pipe with removed pipeline option");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains(&format!("unknown search pipe option: {removed_option}")),
        "{stderr}"
    );
    assert!(
        !marker.exists(),
        "removed pipeline option should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_rejects_unknown_surface_without_provider_spawn() {
    let root = temp_project_root("search-pipe-rejects-unknown-surface");
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
            "--surface",
            "owner,commands",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe with unknown surface");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("unknown search surface: commands"),
        "{stderr}"
    );
    assert!(
        !marker.exists(),
        "unknown surface should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}
