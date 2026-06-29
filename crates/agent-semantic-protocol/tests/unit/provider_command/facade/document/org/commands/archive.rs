use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_org_archive_done_moves_done_records_under_archives() {
    let root = temp_project_root("org-document-command-archive-done");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    let done = plans.join("done-plan.org");
    let todo = plans.join("todo-plan.org");
    std::fs::write(&done, "* DONE Finished plan\nArchive me.\n").expect("write done plan");
    std::fs::write(&todo, "* TODO Open plan\nKeep me active.\n").expect("write todo plan");

    let output = asp_command(&root)
        .args(["org", "archive", "done"])
        .output()
        .expect("run asp org archive done");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("archive stdout");
    assert!(stdout.contains("[ASP_ORG_ARCHIVE] done"), "{stdout}");
    assert!(stdout.contains("archived-count: 1"), "{stdout}");
    assert!(stdout.contains("flow/plans/done-plan.org"), "{stdout}");

    let archived = org_artifacts
        .join("archives")
        .join("flow")
        .join("plans")
        .join("done-plan.org");
    assert!(!done.exists(), "DONE source should be moved");
    assert!(todo.exists(), "TODO source should stay active");
    let archived_text = std::fs::read_to_string(&archived).expect("read archived plan");
    assert!(
        archived_text.contains("#+ARCHIVED_FROM: flow/plans/done-plan.org"),
        "{archived_text}"
    );
    assert!(
        archived_text.contains("#+ARCHIVE_REASON: done"),
        "{archived_text}"
    );
    assert!(
        archived_text.contains("* DONE Finished plan"),
        "{archived_text}"
    );

    let _ = std::fs::remove_dir_all(root);
}
