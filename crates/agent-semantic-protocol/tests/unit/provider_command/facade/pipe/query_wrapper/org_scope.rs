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
    assert!(!stdout.contains("rankedEvidence="), "{stdout}");
    assert!(!stdout.contains("evidenceFrontier="), "{stdout}");
    assert!(!stdout.contains("actionFrontier="), "{stdout}");
    assert!(
        stdout.contains("packages=docs/source-index.org"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "nextCommand=asp fd -query 'State|layout|materialization|Runtime|config-owned' --workspace docs/source-index.org"
        ),
        "{stdout}"
    );
    assert!(
        !stdout.contains("noOutput reason=no-candidates"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_rg_query_reads_yaml_workflow_scope() {
    let root = temp_project_root("asp-rg-query-wrapper-yaml-workflow-scope");
    std::fs::create_dir_all(root.join(".github/workflows")).expect("create workflows");
    std::fs::write(
        root.join(".github/workflows/ci.yml"),
        "name: CI\njobs:\n  test:\n    steps:\n      - run: cargo clippy --workspace\n",
    )
    .expect("write workflow");

    let output = asp_command(&root)
        .args(["rg", "-query", "cargo clippy|workspace", ".github"])
        .output()
        .expect("run asp rg -query against workflow dir");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("sourceTrace=finder:used["), "{stdout}");
    assert!(stdout.contains("workflows/ci.yml"), "{stdout}");
    assert!(
        stdout.contains("nextClasses=fd-query,scoped-rg-query,owner-items"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("noOutput reason=no-candidates"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
