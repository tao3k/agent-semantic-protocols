use crate::provider_command::support::{
    asp_command, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn julia_project_toml_owner_search_rejects_non_source_owner_before_provider_spawn() {
    let root = temp_project_root("julia-project-toml-owner-search");
    let bin_dir = root.join(".bin");
    let provider_marker = root.join("julia-provider-called");
    std::fs::write(root.join("Project.toml"), "name = \"Demo\"\n").expect("write Project.toml");
    write_marker_provider(&bin_dir, "asp-julia-harness", &provider_marker);
    write_activation(&root, &[provider("julia", Vec::new())]);

    let output = asp_command(&root)
        .args([
            "julia",
            "search",
            "owner",
            "Project.toml",
            "items",
            "--query",
            "demo",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp julia Project.toml search");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("code=owner-language-mismatch"), "{stderr}");
    assert!(
        stderr.contains("nextAction=select-language-for-owner-extension"),
        "{stderr}"
    );
    assert!(
        !provider_marker.exists(),
        "non-source owner spawned provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn julia_project_toml_owner_search_rejects_with_explicit_state_home_activation() {
    let root = temp_project_root("julia-project-toml-owner-search-state-home");
    let bin_dir = root.join(".bin");
    let provider_marker = root.join("julia-provider-called");
    std::fs::write(root.join("Project.toml"), "name = \"Demo\"\n").expect("write Project.toml");
    write_marker_provider(&bin_dir, "asp-julia-harness", &provider_marker);
    write_activation(&root, &[provider("julia", Vec::new())]);

    let output = asp_command(&root)
        .args([
            "julia",
            "search",
            "owner",
            "Project.toml",
            "items",
            "--query",
            "demo",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp julia Project.toml search");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("code=owner-language-mismatch"), "{stderr}");
    assert!(
        !provider_marker.exists(),
        "non-source owner spawned provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn julia_missing_project_toml_owner_search_reports_owner_not_found() {
    let root = temp_project_root("julia-missing-project-toml-owner-search");
    let bin_dir = root.join(".bin");
    let provider_marker = root.join("julia-provider-called");
    write_marker_provider(&bin_dir, "asp-julia-harness", &provider_marker);
    write_activation(&root, &[provider("julia", Vec::new())]);

    let output = asp_command(&root)
        .args([
            "julia",
            "search",
            "owner",
            "Project.toml",
            "items",
            "--query",
            "demo",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp julia missing Project.toml search");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("code=invalid-owner"), "{stderr}");
    assert!(stderr.contains("reason=missing-owner"), "{stderr}");
    assert!(
        stderr.contains("nextCommand=asp julia search pipe"),
        "{stderr}"
    );
    assert!(!provider_marker.exists(), "missing owner spawned provider");
    let _ = std::fs::remove_dir_all(root);
}
