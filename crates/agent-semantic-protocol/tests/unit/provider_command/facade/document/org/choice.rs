use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_org_capture_plan_defers_until_specification_choice() {
    let root = temp_project_root("org-document-command-plan-choice");
    let target = ".cache/agent-semantic-protocol/artifacts/org/flow/plans/agent-plan-choice.org";

    let compact = asp_command(&root)
        .args([
            "org",
            "capture",
            "--contract",
            "agent.plan.v1",
            "--title",
            "Choose ASP org plan specification",
            "--target-file",
            target,
        ])
        .output()
        .expect("run asp org capture plan compact choice");
    assert!(
        compact.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&compact.stderr)
    );
    let compact_stdout = String::from_utf8(compact.stdout).expect("compact stdout");
    assert!(
        compact_stdout.contains(
            "[agent-interactive] contract=agent.plan.v1 id=specification method=choice stage=pre-capture target=agent.plan.v1 create=deferred"
        ),
        "{compact_stdout}"
    );
    assert!(
        compact_stdout.contains("status=interactive-required entry=not-created"),
        "{compact_stdout}"
    );
    assert!(
        compact_stdout.contains("load: --choice specification=?"),
        "{compact_stdout}"
    );
    assert!(
        compact_stdout.contains("next: choose --choice specification=N|ID | ask-user"),
        "{compact_stdout}"
    );
    assert!(
        compact_stdout.contains(
            "guard: resolve this interactive window before capture materializes; do not default or use --help"
        ),
        "{compact_stdout}"
    );
    assert!(
        compact_stdout.contains("categories: 1=SDD,2=BDD,3=TDD,4=BDR,5=TASK,?=detail"),
        "{compact_stdout}"
    );
    assert!(
        !compact_stdout.contains("[CAPTURE] asp org capture"),
        "{compact_stdout}"
    );

    let detail = asp_command(&root)
        .args([
            "org",
            "capture",
            "--contract",
            "agent.plan.v1",
            "--title",
            "Choose ASP org plan specification",
            "--target-file",
            target,
            "--choice",
            "specification=?",
        ])
        .output()
        .expect("run asp org capture plan detail choice");
    assert!(
        detail.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&detail.stderr)
    );
    let detail_stdout = String::from_utf8(detail.stdout).expect("detail stdout");
    assert!(
        detail_stdout.contains(
            "[agent-interactive-detail] id=specification method=choice stage=pre-capture target=agent.plan.v1 create=deferred"
        ),
        "{detail_stdout}"
    );
    assert!(
        detail_stdout.contains("next: choose --choice specification=N|ID | ask-user"),
        "{detail_stdout}"
    );
    assert!(
        detail_stdout.contains("guard: choose only with task-specific confidence"),
        "{detail_stdout}"
    );
    assert!(
        detail_stdout.contains(
            "|1|SDD|agent.sdd.v1|Specification-Driven Development|scope/design/API/schema boundary changes|"
        ),
        "{detail_stdout}"
    );
    assert!(
        detail_stdout.contains("|5|TASK||Task Plan|plain checklist/reflection is enough|"),
        "{detail_stdout}"
    );
    assert!(
        !detail_stdout.contains("[CAPTURE] asp org capture"),
        "{detail_stdout}"
    );

    let invalid = asp_command(&root)
        .args([
            "org",
            "capture",
            "--contract",
            "agent.plan.v1",
            "--title",
            "Choose ASP org plan specification",
            "--target-file",
            target,
            "--choice",
            "specification=framework",
        ])
        .output()
        .expect("run asp org capture plan invalid choice");
    assert!(!invalid.status.success(), "invalid choice should fail");
    let invalid_stderr = String::from_utf8(invalid.stderr).expect("invalid stderr");
    assert!(
        invalid_stderr.contains("invalid agent.plan.v1 choice `specification=framework`"),
        "{invalid_stderr}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_capture_plan_records_selected_specification_contract() {
    let root = temp_project_root("org-document-command-plan-spec-contract");

    for (choice, suffix, contract) in [
        ("1", "sdd", "agent.sdd.v1"),
        ("2", "bdd", "agent.bdd.v1"),
        ("3", "tdd", "agent.tdd.v1"),
        ("4", "bdr", "agent.bdr.v1"),
    ] {
        let mut command = asp_command(&root);
        command
            .args([
                "org",
                "capture",
                "--contract",
                "agent.plan.v1",
                "--title",
                "Design ASP org specification plan",
                "--target-file",
            ])
            .arg(format!(
                ".cache/agent-semantic-protocol/artifacts/org/flow/plans/agent-plan-{suffix}-test.org"
            ))
            .arg("--choice")
            .arg(format!("specification={choice}"));
        let output = command
            .output()
            .expect("run asp org capture specification-governed plan");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("capture spec plan stdout");
        assert!(
            stdout.contains(&format!(":GOVERNING_CONTRACT: {contract}")),
            "{stdout}"
        );
        assert!(!stdout.contains(":GOVERNING_REF:"), "{stdout}");
    }

    let task = asp_command(&root)
        .args([
            "org",
            "capture",
            "--contract",
            "agent.plan.v1",
            "--title",
            "Track ASP org implementation task",
            "--target-file",
            ".cache/agent-semantic-protocol/artifacts/org/flow/plans/agent-plan-task-test.org",
            "--choice",
            "specification=5",
        ])
        .output()
        .expect("run asp org capture TASK plan");
    assert!(
        task.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&task.stderr)
    );
    let task_stdout = String::from_utf8(task.stdout).expect("capture task plan stdout");
    assert!(
        !task_stdout.contains(":GOVERNING_CONTRACT:"),
        "{task_stdout}"
    );
    assert!(!task_stdout.contains(":GOVERNING_REF:"), "{task_stdout}");

    let _ = std::fs::remove_dir_all(root);
}
