use crate::provider_command::support::{
    asp_command, provider, temp_project_root, write_activation, write_echo_provider,
};

#[test]
fn gerbil_search_guide_resolves_repo_relative_provider_bin_with_workspace() {
    let root = temp_project_root("search-guide-gerbil-workspace-bin");
    let workspace = root
        .join("languages")
        .join("gerbil-scheme-language-project-harness");
    let provider_bin_dir = workspace.join("bin");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    write_echo_provider(&provider_bin_dir, "gslph", "gerbil");
    std::fs::write(
        root.join("asp.toml"),
        "[languages.gerbil-scheme]\nbin = \"languages/gerbil-scheme-language-project-harness/bin/gslph\"\n",
    )
    .expect("write asp.toml");
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
fn gerbil_search_structural_json_uses_activation_provider_prefix_with_workspace() {
    let root = temp_project_root("search-structural-gerbil-provider-prefix");
    let workspace = root
        .join("languages")
        .join("gerbil-scheme-language-project-harness");
    let provider_bin_dir = workspace.join("bin");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    write_echo_provider(&provider_bin_dir, "gslph", "gerbil");
    write_activation(
        &root,
        &[provider(
            "gerbil-scheme",
            vec![provider_bin_dir.join("gslph").display().to_string()],
        )],
    );

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
