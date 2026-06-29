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

    let capture_task = asp_command(&root)
        .args([
            "org",
            "capture",
            "--contract",
            "agent.task.v1",
            "--title",
            "Record ASP org task",
            "--target-file",
            "TASKS.org",
        ])
        .output()
        .expect("run asp org capture");
    assert!(
        capture_task.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&capture_task.stderr)
    );
    let capture_task_stdout = String::from_utf8(capture_task.stdout).expect("capture task stdout");
    assert!(
        capture_task_stdout.contains("[CAPTURE] asp org capture"),
        "{capture_task_stdout}"
    );
    assert!(
        capture_task_stdout.contains("target-file: TASKS.org"),
        "{capture_task_stdout}"
    );
    assert!(
        capture_task_stdout.contains("* TODO Record ASP org task :task:"),
        "{capture_task_stdout}"
    );
    assert!(
        capture_task_stdout.contains(":CONTRACT_ORG: agent.task.v1"),
        "{capture_task_stdout}"
    );
    assert!(
        capture_task_stdout.contains(":ID: record-asp-org-task"),
        "{capture_task_stdout}"
    );
    assert!(
        capture_task_stdout.contains("** Goal"),
        "{capture_task_stdout}"
    );
    assert!(
        capture_task_stdout.contains("** Acceptance"),
        "{capture_task_stdout}"
    );
    assert!(
        capture_task_stdout.contains("contract-check:"),
        "{capture_task_stdout}"
    );
    assert!(
        capture_task_stdout.contains("- contract: agent.task.v1"),
        "{capture_task_stdout}"
    );
    assert!(
        capture_task_stdout.contains("- status: passed"),
        "{capture_task_stdout}"
    );
    assert!(
        capture_task_stdout.contains("- nonMutating:"),
        "{capture_task_stdout}"
    );
    assert!(
        !root.join("TASKS.org").exists(),
        "asp org capture must not create TASKS.org"
    );

    let capture_plan = asp_command(&root)
        .args([
            "org",
            "capture",
            "--contract",
            "agent.plan.v1",
            "--title",
            "Record ASP org plan",
            "--target-file",
            ".cache/agent-semantic-protocol/artifacts/org/flow/plans/agent-plan-session-test.org",
            "--choice",
            "specification=5",
        ])
        .output()
        .expect("run asp org capture plan");
    assert!(
        capture_plan.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&capture_plan.stderr)
    );
    let capture_plan_stdout = String::from_utf8(capture_plan.stdout).expect("capture plan stdout");
    assert!(
        capture_plan_stdout.contains("* TODO Record ASP org plan [1/7] [14%] :agent:plan:"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains(":ID: record-asp-org-plan"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains(":OBJECTIVE: Record ASP org plan"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains("** Context"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains("** Checkpoints"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains("** Scope And Boundaries"),
        "{capture_plan_stdout}"
    );
    assert!(
        !capture_plan_stdout.contains("Specification Applicability"),
        "{capture_plan_stdout}"
    );
    assert!(
        !capture_plan_stdout.contains("GOVERNING_CONTRACT"),
        "{capture_plan_stdout}"
    );
    assert!(
        !capture_plan_stdout.contains("GOVERNING_REF"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains("** Validation"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains("Capture rendered and =agent.plan.v1= contract check passed."),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains("** Evidence Loop"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains("| Claim | Evidence | Command | Result |"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains("** Reflection"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains("| Did project scope drift? | pending |"),
        "{capture_plan_stdout}"
    );
    assert!(
        !capture_plan_stdout.contains("agent.plan.v1 defaults"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains("** Recovery"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains("- contract: agent.plan.v1"),
        "{capture_plan_stdout}"
    );
    assert!(
        capture_plan_stdout.contains("- status: passed"),
        "{capture_plan_stdout}"
    );
    assert!(
        !root
            .join(".cache")
            .join("agent-semantic-protocol")
            .join("artifacts")
            .join("org")
            .join("flow")
            .join("plans")
            .join("agent-plan-session-test.org")
            .exists(),
        "asp org capture must not create plan file"
    );

    for (target_file, message) in [
        (
            ".cache/agent-semantic-protocol/artifacts/org/current-agent-task.org",
            "agent.plan.v1 --target-file filename must match `agent-plan-*.org`",
        ),
        (
            ".cache/agent-semantic-protocol/artifacts/org/agent-plan-session-test.org",
            "agent.plan.v1 --target-file must be stored under an `org/flow/plans/` path",
        ),
        (
            ".cache/agent-semantic-protocol/artifacts/org/flow/plans/archive/agent-plan-session-test.org",
            "agent.plan.v1 --target-file must be stored under an `org/flow/plans/` path",
        ),
    ] {
        let invalid_capture_plan = asp_command(&root)
            .args([
                "org",
                "capture",
                "--contract",
                "agent.plan.v1",
                "--title",
                "Invalid ASP org plan target",
                "--target-file",
                target_file,
            ])
            .output()
            .expect("run invalid asp org capture plan");
        assert!(
            !invalid_capture_plan.status.success(),
            "agent.plan.v1 target-file should reject {target_file}"
        );
        let stderr =
            String::from_utf8(invalid_capture_plan.stderr).expect("invalid capture plan stderr");
        assert!(stderr.contains(message), "{stderr}");
    }

    let missing_title_plan = asp_command(&root)
        .args([
            "org",
            "capture",
            "--contract",
            "agent.plan.v1",
            "--target-file",
            ".cache/agent-semantic-protocol/artifacts/org/flow/plans/agent-plan-missing-title.org",
        ])
        .output()
        .expect("run missing title asp org capture plan");
    assert!(
        !missing_title_plan.status.success(),
        "agent.plan.v1 should require a recall title"
    );
    let missing_title_stderr =
        String::from_utf8(missing_title_plan.stderr).expect("missing title stderr");
    assert!(
        missing_title_stderr.contains("agent.plan.v1 capture requires `--title`"),
        "{missing_title_stderr}"
    );

    let placeholder_title_plan = asp_command(&root)
        .args([
            "org",
            "capture",
            "--contract",
            "agent.plan.v1",
            "--title",
            "Agent session plan",
            "--target-file",
            ".cache/agent-semantic-protocol/artifacts/org/flow/plans/agent-plan-placeholder-title.org",
        ])
        .output()
        .expect("run placeholder title asp org capture plan");
    assert!(
        !placeholder_title_plan.status.success(),
        "agent.plan.v1 should reject generic session titles"
    );
    let placeholder_title_stderr =
        String::from_utf8(placeholder_title_plan.stderr).expect("placeholder title stderr");
    assert!(
        placeholder_title_stderr.contains("must be a task-specific recall title"),
        "{placeholder_title_stderr}"
    );

    let recall_help = asp_command(&root)
        .args(["org", "recall", "--help"])
        .output()
        .expect("run asp org recall help");
    assert!(
        recall_help.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&recall_help.stderr)
    );
    let recall_help_stdout = String::from_utf8(recall_help.stdout).expect("recall help stdout");
    assert!(
        recall_help_stdout.contains("usage: asp org recall plans"),
        "{recall_help_stdout}"
    );
    assert!(
        recall_help_stdout.contains("Python memory runtime owns plan ranking"),
        "{recall_help_stdout}"
    );

    let capture_task_kind = asp_command(&root)
        .args([
            "org",
            "capture",
            "task",
            "--kind",
            "task",
            "--title",
            "Record ASP org task by kind",
            "--body",
            task_contract_body(),
            "--target-file",
            "TASK_KIND.org",
            "--outline",
            "Plans/Active",
            "--tag",
            "task",
        ])
        .output()
        .expect("run asp org capture positional fallback");
    assert!(
        !capture_task_kind.status.success(),
        "asp org capture task must not resolve agent.task.v1 implicitly"
    );
    let capture_task_kind_stderr =
        String::from_utf8(capture_task_kind.stderr).expect("capture task kind stderr");
    assert!(
        capture_task_kind_stderr.contains("expects `--contract CONTRACT_ID`"),
        "{capture_task_kind_stderr}"
    );
    assert!(
        !capture_task_kind_stderr.contains("--kind task"),
        "{capture_task_kind_stderr}"
    );

    let missing_contract_capture = asp_command(&root)
        .args(["org", "capture", "--kind", "task"])
        .output()
        .expect("run asp org capture without contract");
    assert!(
        !missing_contract_capture.status.success(),
        "asp org capture without --contract should fail"
    );
    let missing_contract_stderr =
        String::from_utf8(missing_contract_capture.stderr).expect("missing contract stderr");
    assert!(
        missing_contract_stderr.contains("expects `--contract CONTRACT_ID`"),
        "{missing_contract_stderr}"
    );
    assert!(
        !missing_contract_stderr.contains("--kind task"),
        "{missing_contract_stderr}"
    );

    let capture_help = asp_command(&root)
        .args(["org", "capture", "--help"])
        .output()
        .expect("run asp org capture help");
    assert!(
        capture_help.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&capture_help.stderr)
    );
    let capture_help_stdout = String::from_utf8(capture_help.stdout).expect("capture help stdout");
    assert!(
        capture_help_stdout.contains("usage: asp org capture --contract CONTRACT_ID"),
        "{capture_help_stdout}"
    );
    assert!(
        !capture_help_stdout.contains("capture init"),
        "{capture_help_stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_capture_auto_initializes_state_resources_and_flow_dirs() {
    let root = temp_project_root("org-document-command-capture-auto-init");

    let output = asp_command(&root)
        .args([
            "org",
            "capture",
            "--contract",
            "agent.plan.v1",
            "--title",
            "Auto initialize ASP org resources",
            "--target-file",
            ".cache/agent-semantic-protocol/artifacts/org/flow/plans/agent-plan-auto-init.org",
            "--choice",
            "specification=TASK",
            "--no-confirm",
        ])
        .output()
        .expect("run asp org capture with automatic state init");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("capture stdout");
    assert!(stdout.contains("[CAPTURE] asp org capture"), "{stdout}");
    assert!(stdout.contains("- contract: agent.plan.v1"), "{stdout}");
    assert!(stdout.contains("- status: passed"), "{stdout}");

    let state_root = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("org");
    assert!(
        state_root
            .join("templates")
            .join("ASP_ORG_SKILL.org")
            .is_file(),
        "ASP_ORG_SKILL.org should be materialized"
    );
    assert!(
        state_root
            .join("templates")
            .join("agent.plan.v1.org")
            .is_file(),
        "plan template should be materialized"
    );
    assert!(
        state_root
            .join("templates")
            .join("agent.execplan.v1.org")
            .is_file(),
        "execplan template should be materialized"
    );
    assert!(
        state_root
            .join("contracts")
            .join("agent.plan.v1.org")
            .is_file(),
        "plan contract should be materialized"
    );
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    assert!(org_artifacts.join("flow").join("plans").is_dir());
    assert!(org_artifacts.join("flow").join("sdd").is_dir());
    assert!(org_artifacts.join("flow").join("bdd").is_dir());
    assert!(org_artifacts.join("flow").join("tdd").is_dir());
    assert!(org_artifacts.join("flow").join("bdr").is_dir());

    let init_output = asp_command(&root)
        .args(["org", "capture", "init"])
        .output()
        .expect("run removed asp org capture init");
    assert!(
        !init_output.status.success(),
        "asp org capture init must not remain public"
    );
    let init_stderr = String::from_utf8(init_output.stderr).expect("capture init stderr");
    assert!(
        init_stderr.contains("asp org capture init is not a public command"),
        "{init_stderr}"
    );

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

fn task_contract_body() -> &'static str {
    "** Goal\nUse asp org capture before applying an Org edit.\n** Acceptance\n- [X] Contract check passes before org-entry is returned.\n** Progress\n- [X] Capture command rendered the entry.\n** Evidence\n- asp org capture --contract agent.task.v1"
}
