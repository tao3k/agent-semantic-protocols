use crate::provider_command::support::{
    asp_command, install_state_home_provider, provider, temp_project_root, write_activation,
    write_echo_provider,
};

#[test]
fn gerbil_search_guide_uses_state_home_provider_with_workspace() {
    let root = temp_project_root("search-guide-gerbil-state-home");
    let workspace = root
        .join("languages")
        .join("gerbil-scheme-language-project-harness");
    let provider_bin_dir = workspace.join("bin");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    write_echo_provider(&provider_bin_dir, "gslph", "gerbil");
    install_state_home_provider(&root, "gerbil-scheme", &provider_bin_dir.join("gslph"));
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env_remove("PATH")
        .args([
            "gerbil-scheme",
            "search",
            "guide",
            "--workspace",
            "languages/gerbil-scheme-language-project-harness",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp gerbil-scheme search guide");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "gerbil args=[search][guide][--view][seeds]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_search_structural_json_uses_state_home_provider_with_workspace() {
    let root = temp_project_root("search-structural-gerbil-state-home");
    let workspace = root
        .join("languages")
        .join("gerbil-scheme-language-project-harness");
    let provider_bin_dir = workspace.join("bin");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    write_echo_provider(&provider_bin_dir, "gslph", "gerbil");
    install_state_home_provider(&root, "gerbil-scheme", &provider_bin_dir.join("gslph"));
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env_remove("PATH")
        .args([
            "gerbil-scheme",
            "search",
            "structural",
            "--json",
            "--workspace",
            "languages/gerbil-scheme-language-project-harness",
        ])
        .output()
        .expect("run asp gerbil-scheme search structural");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "gerbil args=[search][structural][--json]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}
