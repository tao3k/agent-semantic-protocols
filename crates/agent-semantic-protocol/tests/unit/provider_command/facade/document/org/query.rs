use crate::provider_command::support::{asp_command, temp_project_root};

use crate::provider_command::facade::document::support::{
    asp_org_query, write_org_elements_fixture,
};

#[test]
fn org_facade_query_covers_org_element_kinds() {
    let root = temp_project_root("org-document-elements-query");
    let path = write_org_elements_fixture(&root);

    for (kind, row, source_kind) in [
        ("heading", "|heading", "sourceKind=\"Headline\""),
        ("property", "|property", "sourceKind=\"PropertyDrawer\""),
        ("planning", "|planning", "sourceKind=\"SyntaxPlanning\""),
        ("table", "|table", "sourceKind=\"OrgTable\""),
        ("paragraph", "|paragraph", "sourceKind=\"Paragraph\""),
        ("block", "|block", "sourceKind=\"SourceBlock\""),
        ("list", "|list", "sourceKind=\"SyntaxList\""),
        ("task", "|task", "sourceKind=\"Headline\""),
        ("listItem", "|listItem", "sourceKind=\"SyntaxListItem\""),
        ("link", "|link", "sourceKind=\"SyntaxLink\""),
        ("image", "|image", "sourceKind=\"SyntaxLink\""),
    ] {
        let stdout = asp_org_query(
            &root,
            &[
                "query",
                "--kind",
                kind,
                "--workspace",
                ".",
                "--view",
                "metadata",
            ],
        );
        assert!(stdout.contains(row), "kind={kind} stdout={stdout}");
        assert!(stdout.contains(source_kind), "kind={kind} stdout={stdout}");
    }

    let property = asp_org_query(
        &root,
        &[
            "query",
            "--field",
            "key=CUSTOM_ID",
            "--workspace",
            ".",
            "--view",
            "metadata",
        ],
    );
    assert!(property.contains("|property"), "{property}");
    assert!(property.contains("value=\"task-1\""), "{property}");

    let source_block = asp_org_query(
        &root,
        &[
            "query",
            "--field",
            "kind=source",
            "--workspace",
            ".",
            "--view",
            "metadata",
        ],
    );
    assert!(source_block.contains("|block"), "{source_block}");
    assert!(source_block.contains("lang=\"rust\""), "{source_block}");

    let export_block = asp_org_query(
        &root,
        &[
            "query",
            "--field",
            "kind=export",
            "--workspace",
            ".",
            "--view",
            "metadata",
        ],
    );
    assert!(export_block.contains("|block"), "{export_block}");
    assert!(export_block.contains("backend=\"html\""), "{export_block}");

    let paragraph_content = asp_org_query(
        &root,
        &[
            "query",
            "--term",
            "embedded",
            "--workspace",
            ".",
            "--content",
        ],
    );
    assert_eq!(
        paragraph_content.trim(),
        "Provider activation carries execution mode. Document providers stay embedded inside ASP."
    );

    let selector = format!("{}:1-5", path.display());
    let selector_frontier = asp_org_query(
        &root,
        &[
            "query",
            "--selector",
            &selector,
            "--workspace",
            ".",
            "--view",
            "metadata",
        ],
    );
    assert!(selector_frontier.contains("[query-selector] lang=org"));
    assert!(
        selector_frontier.contains("|heading"),
        "{selector_frontier}"
    );
    assert!(
        selector_frontier.contains("|property"),
        "{selector_frontier}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn org_facade_content_projection_deduplicates_list_children() {
    let root = temp_project_root("org-document-content-deduplicates-list");
    let path = write_org_elements_fixture(&root);
    let selector = format!("{}:15-16", path.display());

    let content = asp_org_query(
        &root,
        &[
            "query",
            "--selector",
            &selector,
            "--workspace",
            ".",
            "--content",
        ],
    );

    assert_eq!(content.matches("ship element map").count(), 1, "{content}");
    assert_eq!(content.matches("plain list item").count(), 1, "{content}");
    assert!(content.contains("- [X] ship element map"), "{content}");
    assert!(content.contains("- plain list item"), "{content}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn org_facade_query_excludes_archive_dirs_for_done_artifact_review() {
    let root = temp_project_root("org-document-query-exclude-archive-dir");
    let artifacts = root.join("artifacts").join("org");
    std::fs::create_dir_all(artifacts.join("archives")).expect("create archives dir");
    std::fs::write(artifacts.join("active.org"), "* DONE Active stale task\n")
        .expect("write active org");
    std::fs::write(
        artifacts.join("archives").join("archived.org"),
        "* DONE Archived task\n",
    )
    .expect("write archived org");

    let content = asp_org_query(
        &root,
        &[
            "query",
            "--kind",
            "task",
            "--field",
            "todo=DONE",
            "--exclude-dir",
            "archives",
            "--workspace",
            artifacts.to_str().expect("utf8 artifacts path"),
            "--content",
        ],
    );

    assert!(content.contains("Active stale task"), "{content}");
    assert!(!content.contains("Archived task"), "{content}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn org_facade_search_memory_filters_current_tasks_by_session() {
    let root = temp_project_root("org-document-search-memory-session");
    std::fs::write(
        root.join("plan.org"),
        r#"* TODO Current session task
:PROPERTIES:
:SESSION_ID: session-a
:PLAN_ID: plan-a
:END:

* TODO Other session task
:PROPERTIES:
:SESSION_ID: session-b
:PLAN_ID: plan-b
:END:

* DONE Closed session task
:PROPERTIES:
:SESSION_ID: session-a
:PLAN_ID: plan-a
:END:
"#,
    )
    .expect("write org memory fixture");

    let output = asp_command(&root)
        .args([
            "org",
            "search",
            "memory",
            "--session",
            "session-a",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp org search memory");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");

    assert!(stdout.contains("[search-memory] lang=org"), "{stdout}");
    assert!(stdout.contains("Current session task"), "{stdout}");
    assert!(stdout.contains("session=\"session-a\""), "{stdout}");
    assert!(
        stdout.contains(
            "|python-memory-engine next=asp-memory-engine recall-plan --state .data/omni-memory/state.json --intent 'unfinished org task' --session session-a"
        ),
        "{stdout}"
    );
    assert!(!stdout.contains("Other session task"), "{stdout}");
    assert!(!stdout.contains("Closed session task"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn org_facade_search_memory_defaults_to_codex_thread_id() {
    let root = temp_project_root("org-document-search-memory-codex-session");
    std::fs::write(
        root.join("plan.org"),
        r#"* TODO Current session task
:PROPERTIES:
:SESSION_ID: codex-thread-a
:PLAN_ID: plan-a
:END:

* TODO Other session task
:PROPERTIES:
:SESSION_ID: session-b
:PLAN_ID: plan-b
:END:
"#,
    )
    .expect("write org memory fixture");

    let output = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-thread-a")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .env_remove("AGENT_SESSION_ID")
        .env_remove("SESSION_ID")
        .args([
            "org",
            "search",
            "memory",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp org search memory");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");

    assert!(stdout.contains("[search-memory] lang=org"), "{stdout}");
    assert!(stdout.contains("Current session task"), "{stdout}");
    assert!(stdout.contains("session=\"codex-thread-a\""), "{stdout}");
    assert!(
        stdout.contains(
            "|python-memory-engine next=asp-memory-engine recall-plan --state .data/omni-memory/state.json --intent 'unfinished org task' --session codex-thread-a"
        ),
        "{stdout}"
    );
    assert!(!stdout.contains("Other session task"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn org_facade_search_memory_does_not_default_to_generic_agent_session_env() {
    let root = temp_project_root("org-document-search-memory-no-generic-agent-session");
    std::fs::write(
        root.join("plan.org"),
        r#"* TODO Generic agent session task
:PROPERTIES:
:SESSION_ID: agent-session-a
:PLAN_ID: plan-a
:END:

* TODO Other session task
:PROPERTIES:
:SESSION_ID: session-b
:PLAN_ID: plan-b
:END:
"#,
    )
    .expect("write org memory fixture");

    let output = asp_command(&root)
        .env_remove("CODEX_THREAD_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .env("AGENT_SESSION_ID", "agent-session-a")
        .env("SESSION_ID", "agent-session-a")
        .args([
            "org",
            "search",
            "memory",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp org search memory");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");

    assert!(stdout.contains("Generic agent session task"), "{stdout}");
    assert!(stdout.contains("session=\"agent-session-a\""), "{stdout}");
    assert!(stdout.contains("Other session task"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn org_facade_direct_read_accepts_content_projection_for_hook_recovery() {
    let root = temp_project_root("org-document-direct-read-content");
    std::fs::write(
        root.join("plan.org"),
        "* Guide\n\nHook recovery keeps raw Org source.\n",
    )
    .expect("write org fixture");

    let output = asp_command(&root)
        .args([
            "org",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "plan.org:1-3",
            "--workspace",
            ".",
            "--content",
        ])
        .output()
        .expect("run asp org direct-read content query");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let content = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(content, "* Guide\n\nHook recovery keeps raw Org source.\n");

    let _ = std::fs::remove_dir_all(root);
}
