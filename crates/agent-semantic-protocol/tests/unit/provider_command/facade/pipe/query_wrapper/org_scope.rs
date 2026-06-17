use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_rg_query_reads_explicit_org_file_scope() {
    let root = temp_project_root("asp-rg-query-wrapper-org-scope");
    std::fs::create_dir_all(root.join("docs")).expect("create docs");
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"org-scope\"\n")
        .expect("write cargo manifest");
    std::fs::write(
        root.join("docs/source-index.org"),
        "* State layout materialization\nRuntime materialization uses a config-owned layout.\n",
    )
    .expect("write org doc");

    let output = asp_command(&root)
        .args([
            "rg",
            "-query",
            "State layout materialization|Runtime materialization|config-owned layout",
            "docs/source-index.org",
        ])
        .output()
        .expect("run asp rg -query against explicit org file");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("sourceTrace=finder:used["), "{stdout}");
    assert!(stdout.contains("backend=rg"), "{stdout}");
    assert!(
        stdout.contains("rankedEvidence=H1:docs/source-index.org"),
        "{stdout}"
    );
    assert!(
        stdout.contains("packages=docs/source-index.org"),
        "{stdout}"
    );
    assert!(
        stdout.contains("State layout materialization")
            || stdout.contains("Runtime materialization uses a config-owned layout"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("noOutput reason=no-candidates"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
