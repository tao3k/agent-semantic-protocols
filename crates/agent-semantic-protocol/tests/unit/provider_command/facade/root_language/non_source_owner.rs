use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_echo_provider,
};

#[test]
fn julia_project_toml_owner_query_misses_without_provider_fallback() {
    let root = temp_project_root("julia-project-toml-owner-query-miss");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    std::fs::write(root.join("Project.toml"), "name = \"Demo\"\n").expect("write Project.toml");
    write_echo_provider(&bin_dir, "asp-julia-harness", "julia-provider");
    write_activation(
        &root,
        &[provider(
            "julia",
            vec![bin_dir.join("asp-julia-harness").display().to_string()],
        )],
    );

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args(["julia", "query", "Project.toml", "--query", "demo", "."])
        .output()
        .expect("run asp julia Project.toml query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[search-owner] q=Project.toml"), "{stdout}");
    assert!(stdout.contains("status=miss"), "{stdout}");
    assert!(!stdout.contains("julia-provider args="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}
