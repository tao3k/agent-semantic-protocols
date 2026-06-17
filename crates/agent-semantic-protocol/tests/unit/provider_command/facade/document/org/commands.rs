use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_org_exposes_ast_query_facts_and_capture_plan() {
    let root = temp_project_root("org-document-command-subcommands");
    let path = root.join("sdd.org");
    std::fs::write(&path, asp_org_command_fixture()).expect("write asp org command fixture");

    let sdd_property = asp_command(&root)
        .args([
            "org",
            "query",
            "--kind",
            "property",
            "--field",
            "key=SDD_KIND",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run asp org property query");
    assert!(
        sdd_property.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&sdd_property.stderr)
    );
    let sdd_property_stdout = String::from_utf8(sdd_property.stdout).expect("sdd property stdout");
    assert!(
        sdd_property_stdout.contains("key=\"SDD_KIND\" value=\"capability\""),
        "{sdd_property_stdout}"
    );

    let task = asp_command(&root)
        .args([
            "org",
            "query",
            "--kind",
            "task",
            "--field",
            "todo=TODO",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run asp org task query");
    assert!(
        task.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&task.stderr)
    );
    let task_stdout = String::from_utf8(task.stdout).expect("task stdout");
    assert!(task_stdout.contains("|task"), "{task_stdout}");
    assert!(
        task_stdout.contains("sourceKind=\"Headline\""),
        "{task_stdout}"
    );
    assert!(
        task_stdout.contains("title=\"Capability SDD\""),
        "{task_stdout}"
    );

    let checklist = asp_command(&root)
        .args([
            "org",
            "query",
            "--kind",
            "checklistItem",
            "--field",
            "checked=true",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run asp org checklist query");
    assert!(
        checklist.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&checklist.stderr)
    );
    let checklist_stdout = String::from_utf8(checklist.stdout).expect("checklist stdout");
    assert!(
        checklist_stdout.contains("|checklistItem"),
        "{checklist_stdout}"
    );
    assert!(
        checklist_stdout.contains("sourceKind=\"SyntaxListItem\""),
        "{checklist_stdout}"
    );
    assert!(
        checklist_stdout.contains("checked=\"true\""),
        "{checklist_stdout}"
    );

    let capture_plan = asp_command(&root)
        .args([
            "org",
            "capture-plan",
            "--kind",
            "task",
            "--title",
            "Record ASP org plan",
            "--body",
            "Use asp org capture-plan before applying an Org edit.",
            "--target-file",
            "PLANS.org",
            "--outline",
            "Plans/Active",
            "--tag",
            "plan",
            "--property",
            "PLAN_ID=asp-org-recording",
        ])
        .output()
        .expect("run asp org capture-plan");
    assert!(
        capture_plan.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&capture_plan.stderr)
    );
    let capture_plan_stdout = String::from_utf8(capture_plan.stdout).expect("capture plan stdout");
    assert!(
        capture_plan_stdout.contains("[CAPTURE_PLAN] orgize capture-plan"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains("target-file: PLANS.org"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains("* TODO Record ASP org plan :plan:"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains("- nonMutating:"),
        "{capture_plan_stdout}"
    );
    assert!(
        !root.join("PLANS.org").exists(),
        "asp org capture-plan must not create PLANS.org"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_rejects_domain_specific_embedded_commands() {
    let root = temp_project_root("org-document-command-domain-specific-rejections");

    for command in ["sdd", "agent-planning", "sparse-tree", "task-list"] {
        let output = asp_command(&root)
            .args(["org", command])
            .output()
            .unwrap_or_else(|error| panic!("run asp org {command}: {error}"));
        assert!(
            !output.status.success(),
            "{command} unexpectedly succeeded with stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let stderr = String::from_utf8(output.stderr).expect("stderr");
        assert!(
            stderr.contains(&format!("unsupported document command `{command}`")),
            "command={command} stderr={stderr}"
        );
        let supported = stderr
            .split("supported commands are ")
            .nth(1)
            .unwrap_or_default();
        assert!(!supported.contains("sdd"), "{stderr}");
        assert!(!supported.contains("agent-planning"), "{stderr}");
        assert!(!supported.contains("task-list"), "{stderr}");
        assert!(!supported.contains("sparse-tree"), "{stderr}");
    }

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
- [X] checklist evidence is a checklist item, not a task headline
"#
}
