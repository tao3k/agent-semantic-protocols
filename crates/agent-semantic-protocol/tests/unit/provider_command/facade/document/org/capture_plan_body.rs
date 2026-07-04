use crate::provider_command::support::{asp_command, org_artifact_target, temp_project_root};

#[test]
fn asp_org_capture_plan_body_fills_template_slots_without_replacing_skeleton() {
    let root = temp_project_root("org-capture-plan-body-slots");
    let target = org_artifact_target(&root, "flow/plans/agent-plan-body-slot-test.org");
    let output = asp_command(&root)
        .args([
            "org",
            "capture",
            "--contract",
            "agent.plan.v1",
            "--title",
            "AspGraphsSearch Julia analyzer research integration",
            "--target-file",
            target.as_str(),
            "--choice",
            "specification=TASK",
            "--body",
            "Objective: add tao3k/AspGraphsSearch.jl under analyzers as a submodule and wire the Julia research project around ScienceResearch.jl, Graphs.jl, artifacts search-command analysis, and Pluto-to-HTML publishing. Scope: keep this as analyzer/research infrastructure for ASP language search/query workflow and graph-algorithm exploration, complementing existing Python graph-turbo work rather than duplicating it.",
            "--no-confirm",
        ])
        .output()
        .expect("run asp org capture plan with body slots");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("capture plan body stdout");
    assert!(
        stdout.contains(
            "* TODO AspGraphsSearch Julia analyzer research integration [1/7] [14%] :agent:plan:"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            ":OBJECTIVE: add tao3k/AspGraphsSearch.jl under analyzers as a submodule and wire the Julia research project around ScienceResearch.jl, Graphs.jl, artifacts search-command analysis, and Pluto-to-HTML publishing."
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            ":SCOPE_REF: keep this as analyzer/research infrastructure for ASP language search/query workflow and graph-algorithm exploration, complementing existing Python graph-turbo work rather than duplicating it."
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "- Objective :: add tao3k/AspGraphsSearch.jl under analyzers as a submodule and wire the Julia research project around ScienceResearch.jl, Graphs.jl, artifacts search-command analysis, and Pluto-to-HTML publishing."
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "- Scope reference :: keep this as analyzer/research infrastructure for ASP language search/query workflow and graph-algorithm exploration, complementing existing Python graph-turbo work rather than duplicating it."
        ),
        "{stdout}"
    );
    assert!(stdout.contains("** Recovery"), "{stdout}");
    assert!(stdout.contains("- status: passed"), "{stdout}");
    assert!(
        !stdout.contains("Objective: add tao3k/AspGraphsSearch.jl under analyzers as a submodule"),
        "{stdout}"
    );
    assert!(
        !std::path::Path::new(&target).exists(),
        "asp org capture with body slots must not create plan file"
    );
}

#[test]
fn asp_org_capture_plan_body_records_codex_thread_id_as_session_id() {
    let root = temp_project_root("org-capture-plan-codex-thread");
    let target = org_artifact_target(&root, "flow/plans/agent-plan-thread-scoped.org");
    let output = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-thread-123")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .env_remove("AGENT_SESSION_ID")
        .env_remove("SESSION_ID")
        .env_remove("AGENT_CLIENT")
        .env_remove("SESSION_CLIENT")
        .args([
            "org",
            "capture",
            "--contract",
            "agent.plan.v1",
            "--title",
            "Thread scoped ASP Org plan",
            "--target-file",
            target.as_str(),
            "--choice",
            "specification=TASK",
            "--body",
            "Objective: record Codex thread id on generated plan entries.",
            "--no-confirm",
        ])
        .output()
        .expect("run asp org capture plan with codex thread id");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("capture plan body stdout");
    assert!(stdout.contains(":SESSION_ID: codex-thread-123"), "{stdout}");
    assert!(stdout.contains(":SESSION_CLIENT: codex"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_capture_plan_body_does_not_use_generic_agent_session_env() {
    let root = temp_project_root("org-capture-plan-no-generic-agent-session");
    let target = org_artifact_target(&root, "flow/plans/agent-plan-no-generic-agent-scoped.org");
    let output = asp_command(&root)
        .env_remove("CODEX_THREAD_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .env("AGENT_SESSION_ID", "agent-session-456")
        .env("SESSION_ID", "generic-session-789")
        .env("AGENT_CLIENT", "custom-agent")
        .args([
            "org",
            "capture",
            "--contract",
            "agent.plan.v1",
            "--title",
            "No generic agent scoped ASP Org plan",
            "--target-file",
            target.as_str(),
            "--choice",
            "specification=TASK",
            "--body",
            "Objective: ignore generic agent session env on generated plan entries.",
            "--no-confirm",
        ])
        .output()
        .expect("run asp org capture plan without generic agent session id");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("capture plan body stdout");
    assert!(!stdout.contains("agent-session-456"), "{stdout}");
    assert!(!stdout.contains("generic-session-789"), "{stdout}");
    assert!(!stdout.contains("custom-agent"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}
