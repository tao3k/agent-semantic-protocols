use crate::provider_command::support::{
    asp_command, make_executable, prepend_path, temp_project_root,
};
use std::path::Path;
use std::process::Command;

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
        recall_help_stdout.contains("Python asp-memory-engine owns plan ranking"),
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

#[test]
fn asp_org_recall_plans_scans_in_rust_and_ranks_with_memory_engine() {
    let root = temp_project_root("org-document-command-recall-plans-rank");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    let hot_plan = plans.join("agent-plan-memory-engine-hot-path.org");
    let cold_plan = plans.join("agent-plan-unrelated-cold-path.org");
    std::fs::write(
        &hot_plan,
        "* TODO Stabilize memory engine recall flow [1/8] [12%] :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: memory-engine-hot-path\n:OBJECTIVE: Stabilize memory engine recall flow\n:NEXT_ACTION: continue memory engine sandtable\n:RECOVERY_REF: PLAN_ID=memory-engine-hot-path\n:END:\n",
    )
    .expect("write hot plan");
    std::fs::write(
        &cold_plan,
        "* TODO Unrelated packaging cleanup :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: unrelated-cold-path\n:OBJECTIVE: Unrelated packaging cleanup\n:NEXT_ACTION: continue unrelated cleanup\n:END:\n",
    )
    .expect("write cold plan");
    let state_path = root.join("memory-state.json");
    write_memory_rank_state(&root, &state_path, "memory-engine-hot-path");

    let output = asp_command(&root)
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--state",
            state_path.to_str().unwrap(),
            "--project",
            "repo",
            "--intent",
            "stabilize memory engine recall flow",
            "--top-k",
            "1",
            "--embedding-dim",
            "8",
        ])
        .output()
        .expect("run asp org recall plans");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(
        stdout.contains(
            "[org-recall-plans] owner=rust memoryEngine=asp-memory-engine ranker=memory-engine"
        ),
        "{stdout}"
    );
    assert!(stdout.contains("id=\"memory-engine-hot-path\""), "{stdout}");
    assert!(
        stdout.contains("objective=\"Stabilize memory engine recall flow\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("memoryScore=") && !stdout.contains("id=\"unrelated-cold-path\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("|plan-action rank=1 action=\"resume\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("reason=\"unfinished plan ranked by memory, intent, and recency\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("|next recommendedAction=\"resume\" rank=1"),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_recall_plans_uses_explicit_memory_engine_binary() {
    let root = temp_project_root("org-document-command-recall-plans-binary");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    std::fs::write(
        plans.join("agent-plan-binary-plan.org"),
        "* TODO Binary backed recall plan :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: binary-plan\n:OBJECTIVE: Binary backed recall plan\n:NEXT_ACTION: keep the memory engine on a packaged binary path\n:END:\n",
    )
    .expect("write binary plan");
    let bin_dir = root.join("bin");
    std::fs::create_dir_all(&bin_dir).expect("create binary dir");
    let memory_engine = bin_dir.join("asp-memory-engine-test-binary");
    std::fs::write(
        &memory_engine,
        "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{\"plans\":[{\"id\":\"binary-plan\",\"score\":9.0,\"textScore\":0.0,\"memoryScore\":9.0,\"recencyScore\":0.0,\"intentScore\":0.0}]}'\n",
    )
    .expect("write fake memory engine binary");
    make_executable(&memory_engine);

    let output = asp_command(&root)
        .env("ASP_MEMORY_ENGINE", &memory_engine)
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--project",
            "repo",
            "--intent",
            "binary backed recall plan",
            "--top-k",
            "1",
        ])
        .output()
        .expect("run asp org recall plans with explicit binary");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(stdout.contains("id=\"binary-plan\""), "{stdout}");
    assert!(stdout.contains("score=9.000"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_recall_plans_uses_path_memory_engine_binary() {
    let root = temp_project_root("org-document-command-recall-plans-path-binary");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    std::fs::write(
        plans.join("agent-plan-path-binary-plan.org"),
        "* TODO PATH backed recall plan :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: path-binary-plan\n:OBJECTIVE: PATH backed recall plan\n:NEXT_ACTION: keep packaged asp-memory-engine ahead of development fallbacks\n:END:\n",
    )
    .expect("write path binary plan");
    let bin_dir = root.join("bin");
    std::fs::create_dir_all(&bin_dir).expect("create binary dir");
    let memory_engine = bin_dir.join("asp-memory-engine");
    std::fs::write(
        &memory_engine,
        "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{\"plans\":[{\"id\":\"path-binary-plan\",\"score\":7.0,\"textScore\":0.0,\"memoryScore\":7.0,\"recencyScore\":0.0,\"intentScore\":0.0}]}'\n",
    )
    .expect("write fake memory engine binary");
    make_executable(&memory_engine);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--project",
            "repo",
            "--intent",
            "path backed recall plan",
            "--top-k",
            "1",
        ])
        .output()
        .expect("run asp org recall plans with path binary");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(stdout.contains("id=\"path-binary-plan\""), "{stdout}");
    assert!(stdout.contains("score=7.000"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn asp_org_recall_plans_uses_memory_engine_socket_worker() {
    let root = temp_project_root("org-document-command-recall-plans-socket-worker");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    std::fs::write(
        plans.join("agent-plan-socket-worker-plan.org"),
        "* TODO Socket worker recall plan :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: socket-worker-plan\n:OBJECTIVE: Socket worker recall plan\n:NEXT_ACTION: rank through resident memory worker\n:END:\n",
    )
    .expect("write socket worker plan");
    let socket_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let socket_path = std::path::PathBuf::from(format!(
        "/tmp/asp-memory-worker-{}-{socket_id}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&socket_path);
    let listener =
        std::os::unix::net::UnixListener::bind(&socket_path).expect("bind memory worker socket");
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept memory worker request");
        let mut request = String::new();
        let mut reader = std::io::BufReader::new(stream.try_clone().expect("clone worker stream"));
        std::io::BufRead::read_line(&mut reader, &mut request).expect("read worker request");
        assert!(request.contains("\"command\":\"rank-plans\""), "{request}");
        assert!(request.contains("\"payload\""), "{request}");
        assert!(request.contains("\"socket-worker-plan\""), "{request}");
        std::io::Write::write_all(
            &mut stream,
            b"{\"plans\":[{\"id\":\"socket-worker-plan\",\"score\":6.0,\"textScore\":0.0,\"memoryScore\":6.0,\"recencyScore\":0.0,\"intentScore\":0.0}]}\n",
        )
        .expect("write worker response");
    });

    let output = asp_command(&root)
        .env("ASP_MEMORY_ENGINE_SOCKET", &socket_path)
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--project",
            "repo",
            "--intent",
            "socket worker recall plan",
            "--top-k",
            "1",
        ])
        .output()
        .expect("run asp org recall plans with socket worker");
    handle.join().expect("worker socket thread");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(stdout.contains("id=\"socket-worker-plan\""), "{stdout}");
    assert!(stdout.contains("score=6.000"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_file(socket_path);
}

#[test]
fn asp_org_guide_exposes_generic_ast_recipes_only() {
    let root = temp_project_root("org-document-command-guide-generic");

    let output = asp_command(&root)
        .args(["org", "guide"])
        .output()
        .expect("run asp org guide");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("|recipe todo-tasks=asp org query --kind task --field todo=TODO"),
        "{stdout}"
    );
    assert!(
        stdout.contains("|recipe checked-checklist-items=asp org query --kind checklistItem"),
        "{stdout}"
    );
    assert!(
        stdout.contains("|recipe property-value=asp org query --kind property --field key=<KEY>"),
        "{stdout}"
    );
    assert!(
        stdout.contains("|recipe capture-task=asp org capture --contract agent.task.v1 --title"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|recipe sdd-kind-properties=asp org query --kind property --field key=SDD_KIND"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("|recipe org-id-properties=asp org query --kind property --field key=ID"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|recipe tagged-tasks=asp org query --kind task --term <TEXT> --field tag=<TAG>"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("|recipe done-tasks=asp org query --kind task --field todo=DONE"),
        "{stdout}"
    );

    for domain_recipe in [
        "sdd-property",
        "wendao-task",
        "wendao-orgid",
        "agent-plan",
        "plan-record",
    ] {
        assert!(
            !stdout.contains(domain_recipe),
            "legacy recipe `{domain_recipe}` leaked into asp org guide:\n{stdout}"
        );
    }

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

fn write_memory_rank_state(root: &Path, state_path: &Path, plan_id: &str) {
    let script = root.join("write-memory-state.py");
    std::fs::write(
        &script,
        format!(
            r#"from pathlib import Path
import sys
from asp_memory_engine import Episode, EpisodeDraft, EpisodeStore, PlanMemoryContext, StoreConfig

state = Path(sys.argv[1])
store = EpisodeStore(StoreConfig(path=str(state), embedding_dim=8))
context = PlanMemoryContext(project_id="repo", plan_id="{plan_id}")
store.store(Episode.new(EpisodeDraft(
    id="memory-engine-hot-episode",
    intent="stabilize memory engine recall flow",
    intent_embedding=store.encoder.encode("stabilize memory engine recall flow"),
    experience="continue memory engine sandtable",
    outcome="pending",
).with_plan_context(context, sharing="project")))
store.save_state(state)
"#
        ),
    )
    .expect("write memory state script");
    let packages_python = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/python");
    let output = Command::new("uv")
        .args(["run", "--project"])
        .arg(packages_python)
        .arg("--frozen")
        .arg("python")
        .arg(&script)
        .arg(state_path)
        .current_dir(root)
        .output()
        .expect("run memory state script");
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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

fn task_contract_body() -> &'static str {
    "** Goal\nUse asp org capture before applying an Org edit.\n** Acceptance\n- [X] Contract check passes before org-entry is returned.\n** Progress\n- [X] Capture command rendered the entry.\n** Evidence\n- asp org capture --contract agent.task.v1"
}
