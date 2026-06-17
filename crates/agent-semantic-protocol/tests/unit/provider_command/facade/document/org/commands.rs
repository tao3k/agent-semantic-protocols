use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_org_exposes_embedded_command_subcommands() {
    let root = temp_project_root("org-document-command-subcommands");
    let path = root.join("sdd.org");
    std::fs::write(&path, asp_org_command_fixture()).expect("write asp org command fixture");

    let status = asp_command(&root)
        .args(["org", "sdd", "status", path.to_str().unwrap()])
        .output()
        .expect("run asp org sdd status");
    assert!(
        status.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let status_stdout = String::from_utf8(status.stdout).expect("status stdout");
    assert!(status_stdout.contains("[SDD]"), "{status_stdout}");
    assert!(
        status_stdout.contains("architecture nodes: 2"),
        "{status_stdout}"
    );

    let graph_diff = asp_command(&root)
        .args(["org", "sdd", "graph-diff", path.to_str().unwrap()])
        .output()
        .expect("run asp org sdd graph-diff");
    assert!(
        graph_diff.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&graph_diff.stderr)
    );
    let graph_diff_stdout = String::from_utf8(graph_diff.stdout).expect("graph diff stdout");
    assert_eq!(graph_diff_stdout, "[ok] orgize sdd graph-diff\n");

    let planning = asp_command(&root)
        .args([
            "org",
            "agent-planning",
            "--date",
            "2026-05-14",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run asp org agent-planning");
    assert!(
        planning.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&planning.stderr)
    );
    let planning_stdout = String::from_utf8(planning.stdout).expect("planning stdout");
    assert!(
        planning_stdout.contains("[PLAN006] Action: Scheduled task"),
        "{planning_stdout}"
    );

    let sparse_tree = asp_command(&root)
        .args([
            "org",
            "sparse-tree",
            "--text",
            "routing",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run asp org sparse-tree");
    assert!(
        sparse_tree.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&sparse_tree.stderr)
    );
    let sparse_tree_stdout = String::from_utf8(sparse_tree.stdout).expect("sparse tree stdout");
    assert!(
        sparse_tree_stdout.contains("[SPARSE001] Match: Capability SDD"),
        "{sparse_tree_stdout}"
    );

    let task_list = asp_command(&root)
        .args([
            "org",
            "task-list",
            "--text",
            "routing",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run asp org task-list");
    assert!(
        task_list.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&task_list.stderr)
    );
    let task_list_stdout = String::from_utf8(task_list.stdout).expect("task list stdout");
    assert!(
        task_list_stdout.contains("[TASK_LIST]"),
        "{task_list_stdout}"
    );
    assert!(
        task_list_stdout.contains("- TODO Capability SDD"),
        "{task_list_stdout}"
    );

    let export = asp_command(&root)
        .args(["org", "export", "md", path.to_str().unwrap()])
        .output()
        .expect("run asp org export md");
    assert!(
        export.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    let export_stdout = String::from_utf8(export.stdout).expect("export stdout");
    assert!(export_stdout.contains("# System SDD"), "{export_stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_preserves_embedded_command_failure_status() {
    let root = temp_project_root("org-document-command-failure-status");
    let path = root.join("drift.org");
    std::fs::write(&path, asp_org_drift_fixture()).expect("write drift fixture");

    let output = asp_command(&root)
        .args([
            "org",
            "sdd",
            "graph-diff",
            "--fail-on-drift",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run asp org sdd graph-diff fail");

    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[SDD_GRAPH_DRIFT]"), "{stdout}");
    assert!(stdout.contains("Capability SDD @"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_lint_fix_writes_formatted_org_files() {
    let root = temp_project_root("org-document-command-lint-fix");
    let path = root.join("table.org");
    std::fs::write(&path, "* Table\n|a|bb|\n|long|c|\n").expect("write table fixture");

    let output = asp_command(&root)
        .args(["org", "lint", "--fix", path.to_str().unwrap()])
        .output()
        .expect("run asp org lint --fix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(stdout, "[ok] orgize lint\n");
    let fixed = std::fs::read_to_string(&path).expect("read fixed org");
    assert!(fixed.contains("| a    | bb |"), "{fixed}");
    assert!(fixed.contains("| long | c  |"), "{fixed}");

    let _ = std::fs::remove_dir_all(root);
}

fn asp_org_command_fixture() -> &'static str {
    r#"* System SDD :sdd:
:PROPERTIES:
:ID: 018f3f9c-8d3e-7b2a-9c91-4f5b2e7a2c11
:SDD_KIND: system
:SDD_STATUS: review
:SDD_CONCERN: Routing evidence should stay source-grounded.
:END:
** TODO Capability SDD :sdd:
SCHEDULED: <2026-05-14 Thu>
:PROPERTIES:
:ID: 018f3f9c-7a91-73b4-b3f2-12c4c4d80d77
:SDD_KIND: capability
:SDD_PARENT: [[id:018f3f9c-8d3e-7b2a-9c91-4f5b2e7a2c11][System SDD]]
:SDD_CAPABILITY: semantic-routing
:SDD_STATUS: review
:END:
Routing work is visible to sparse-tree queries.
"#
}

fn asp_org_drift_fixture() -> &'static str {
    r#"* System SDD :sdd:
:PROPERTIES:
:ID: 018f3f9c-8d3e-7b2a-9c91-4f5b2e7a2c11
:SDD_KIND: system
:SDD_STATUS: review
:SDD_CONCERN: Routing evidence should stay source-grounded.
:END:
* Capability SDD :sdd:
:PROPERTIES:
:ID: 018f3f9c-7a91-73b4-b3f2-12c4c4d80d77
:SDD_KIND: capability
:SDD_PARENT: [[id:018f3f9c-8d3e-7b2a-9c91-4f5b2e7a2c11][System SDD]]
:SDD_CAPABILITY: semantic-routing
:SDD_STATUS: review
:END:
"#
}
