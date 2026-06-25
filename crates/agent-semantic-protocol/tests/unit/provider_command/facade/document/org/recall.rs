use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_org_recall_plans_marks_done_records_archive_action() {
    let root = temp_project_root("org-document-command-recall-plans-archive-action");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    std::fs::write(
        plans.join("agent-plan-archive-ready.org"),
        "* DONE Archive ready plan [3/3] [100%] :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: archive-ready-plan\n:OBJECTIVE: Archive ready plan\n:STATUS: complete\n:NEXT_ACTION: archive-ready\n:EVIDENCE_STATUS: validated\n:REVIEW_STATUS: passed\n:END:\n** Reflection\n| Question | Value | Evidence |\n| Did the task finish? | yes | [[#archive-ready-plan][plan evidence]] |\n| Did project scope drift? | no | [[#archive-ready-plan][plan root]] |\n| Are all checklist items done? | yes | [[#archive-ready-plan][plan root]] |\n",
    )
    .expect("write archive ready plan");

    let output = asp_command(&root)
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--include-done",
            "--archive-dir",
            "closed",
            "--intent",
            "archive ready plan",
            "--top-k",
            "1",
        ])
        .output()
        .expect("run asp org recall plans include done");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(stdout.contains("todo=\"DONE\""), "{stdout}");
    assert!(stdout.contains("reflectionComplete=\"true\""), "{stdout}");
    assert!(
        stdout.contains("|plan-action rank=1 action=\"archive\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("reason=\"plan is DONE or archive-ready with completed reflection\""),
        "{stdout}"
    );
    assert!(stdout.contains("--archive-dir closed"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_recall_plans_blocks_archive_until_reflection_answered() {
    let root = temp_project_root("org-document-command-recall-plans-reflection-gate");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    std::fs::write(
        plans.join("agent-plan-needs-reflection.org"),
        "* DONE Needs reflection plan [3/3] [100%] :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: needs-reflection-plan\n:OBJECTIVE: Needs reflection plan\n:STATUS: complete\n:NEXT_ACTION: archive-ready\n:EVIDENCE_STATUS: validated\n:REVIEW_STATUS: passed\n:END:\n** Reflection\n| Question | Value | Evidence |\n| Did the task finish? | pending | [[#needs-reflection-plan][plan evidence]] |\n",
    )
    .expect("write needs reflection plan");

    let output = asp_command(&root)
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--include-done",
            "--intent",
            "needs reflection plan",
            "--top-k",
            "1",
        ])
        .output()
        .expect("run asp org recall plans include done");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(stdout.contains("reflectionComplete=\"false\""), "{stdout}");
    assert!(
        stdout.contains("|plan-action rank=1 action=\"complete-reflection\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("reason=\"reflection answers are required before archive\""),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}
