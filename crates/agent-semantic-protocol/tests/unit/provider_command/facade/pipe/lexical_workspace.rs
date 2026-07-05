use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};
use agent_semantic_protocol::run_cli_args;
use std::time::{Duration, Instant};

const WORKSPACE_FILE_REJECTION_API_MAX: Duration = Duration::from_millis(25);

#[test]
fn lexical_accepts_workspace_and_trailing_scope_path() {
    let root = temp_project_root("search-lexical-workspace-scope");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    let scoped_dir = root.join("packages/python/asp_graph_turbo/src/asp_graph_turbo");
    std::fs::create_dir_all(&scoped_dir).expect("create scoped dir");
    std::fs::create_dir_all(root.join("tests/unit")).expect("create tests dir");
    std::fs::write(
        scoped_dir.join("calibration.py"),
        "def calibration():\n    return 1\n",
    )
    .expect("write scoped source");
    std::fs::write(
        root.join("tests/unit/noise.py"),
        "def calibration():\n    return 2\n",
    )
    .expect("write unscoped source");
    write_marker_provider(&bin_dir, "py-harness", &marker);
    write_activation(&root, &[provider("python", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "python",
            "search",
            "lexical",
            "calibration",
            "--workspace",
            ".",
            "--view",
            "seeds",
            "packages/python/asp_graph_turbo/src/asp_graph_turbo",
        ])
        .output()
        .expect("run asp python search lexical with workspace scope");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[graph-frontier]"), "{stdout}");
    assert!(
        stdout.contains("packages/python/asp_graph_turbo/src/asp_graph_turbo/calibration.py"),
        "{stdout}"
    );
    assert!(!stdout.contains("tests/unit/noise.py"), "{stdout}");
    assert!(!marker.exists(), "search lexical should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn workspace_file_is_rejected_before_provider_spawn() {
    let root = temp_project_root("search-workspace-file-rejected");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::write(root.join("build-std.ss"), "(display \"build\")\n").expect("write owner file");
    write_marker_provider(&bin_dir, "gslph", &marker);
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "search",
            "owner",
            "build-std.ss",
            "items",
            "--query",
            "builded|pended|optimization|make|clan|building",
            "--workspace",
            "build-std.ss",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp gerbil-scheme search with file workspace");

    assert!(
        !output.status.success(),
        "workspace file should fail before provider spawn"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--workspace requires a directory project root"),
        "stderr={stderr}"
    );
    assert!(
        stderr.contains("Keep the file path as the owner/selector"),
        "stderr={stderr}"
    );
    assert!(
        !marker.exists(),
        "provider must not run for invalid workspace"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn workspace_asp_project_state_root_is_rejected_before_provider_spawn() {
    let root = temp_project_root("search-workspace-asp-state-root-rejected");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    let state_project_root = root
        .join(".agent-semantic-protocols")
        .join("projects")
        .join("by-id")
        .join("repo-011d5a105c39176d");
    std::fs::create_dir_all(&state_project_root).expect("create asp state project root");
    std::fs::write(state_project_root.join("project.json"), "{}\n").expect("write project json");
    std::fs::write(root.join("build-std.ss"), "(display \"build\")\n").expect("write owner file");
    write_marker_provider(&bin_dir, "gslph", &marker);
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);
    let state_project_root_arg = state_project_root.display().to_string();

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "search",
            "owner",
            "build-std.ss",
            "items",
            "--query",
            "builded|pended|optimization|make|clan|building",
            "--workspace",
            state_project_root_arg.as_str(),
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp gerbil-scheme search with asp state root workspace");

    assert!(
        !output.status.success(),
        "ASP state root workspace should fail before provider spawn"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ASP project state root"), "stderr={stderr}");
    assert!(stderr.contains("projects/by-id"), "stderr={stderr}");
    assert!(
        stderr.contains("real checkout workspace"),
        "stderr={stderr}"
    );
    assert!(
        !marker.exists(),
        "provider must not run for invalid workspace"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn state_projects_audit_distinguishes_active_present_and_orphan_projects() {
    let root = temp_project_root("state-projects-audit-classification");
    let state_home = root.join(".asp-home");
    let projects_dir = state_home.join("projects").join("by-id");
    std::fs::create_dir_all(&projects_dir).expect("create projects dir");

    let checkout_base = std::env::current_dir()
        .expect("current dir")
        .join("target")
        .join("state-projects-audit-fixtures")
        .join(root.file_name().expect("fixture root name"));
    let _ = std::fs::remove_dir_all(&checkout_base);
    let active_checkout = checkout_base.join("active-repo");
    let duplicate_checkout = checkout_base.join("duplicate-repo");
    let research_checkout = checkout_base.join(".data").join("loopx");
    let nested_checkout = checkout_base
        .join("agent-semantic-protocols")
        .join("languages")
        .join("rust");
    let no_remote_checkout = checkout_base.join("no-remote");
    for checkout in [
        &active_checkout,
        &duplicate_checkout,
        &research_checkout,
        &nested_checkout,
        &no_remote_checkout,
    ] {
        std::fs::create_dir_all(checkout).expect("create checkout fixture");
    }

    let write_project =
        |repo_id: &str, checkout_root: &std::path::Path, remote_url: Option<&str>| {
            let project_dir = projects_dir.join(repo_id);
            std::fs::create_dir_all(&project_dir).expect("create project state dir");
            let body = serde_json::json!({
                "stateLayoutVersion": "state-v2",
                "repoId": repo_id,
                "displayName": repo_id,
                "checkoutRoot": checkout_root,
                "gitToplevel": checkout_root,
                "gitDir": checkout_root.join(".git"),
                "gitCommonDir": checkout_root.join(".git"),
                "remoteUrl": remote_url,
                "identityBasis": remote_url.map(|remote| format!("git-remote:{remote}")),
            });
            std::fs::write(
                project_dir.join("project.json"),
                serde_json::to_string_pretty(&body).expect("serialize project json"),
            )
            .expect("write project json");
        };

    write_project(
        "repo-active",
        &active_checkout,
        Some("git@github.com:example/active.git"),
    );
    write_project(
        "repo-duplicate",
        &duplicate_checkout,
        Some("https://github.com/example/active"),
    );
    write_project(
        "repo-research",
        &research_checkout,
        Some("https://github.com/example/research.git"),
    );
    write_project(
        "repo-nested",
        &nested_checkout,
        Some("https://github.com/example/nested.git"),
    );
    write_project("repo-no-remote", &no_remote_checkout, None);
    write_project(
        "repo-missing-checkout",
        &root.join("missing-checkout"),
        Some("https://github.com/example/missing.git"),
    );
    std::fs::create_dir_all(projects_dir.join("repo-missing-project-json"))
        .expect("create missing project json fixture");

    let output = asp_command(&root)
        .env("ASP_STATE_HOME", &state_home)
        .args(["state", "projects", "audit", "--json"])
        .output()
        .expect("run asp state projects audit");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse state projects audit json");
    assert_eq!(report["summary"]["total"], 7);
    assert_eq!(report["summary"]["activeCandidate"], 1);
    assert_eq!(report["summary"]["activeUniqueRemote"], 1);
    assert_eq!(report["summary"]["presentCandidate"], 3);
    assert_eq!(report["summary"]["orphanCandidate"], 3);
    assert_eq!(report["summary"]["missingProjectJson"], 1);
    assert_eq!(report["summary"]["missingCheckout"], 1);
    assert_eq!(report["summary"]["noRemote"], 1);
    assert_eq!(report["summary"]["researchCheckout"], 1);
    assert_eq!(report["summary"]["nestedCheckout"], 1);
    assert_eq!(report["summary"]["duplicateRemote"], 2);

    let projects = report["projects"].as_array().expect("projects array");
    let array_has = |project: &serde_json::Value, key: &str, needle: &str| {
        project[key]
            .as_array()
            .expect("json array")
            .iter()
            .any(|value| value.as_str() == Some(needle))
    };
    let active = projects
        .iter()
        .find(|project| project["repoId"] == "repo-active")
        .expect("active project");
    assert_eq!(active["status"], "active_candidate");
    assert_eq!(active["activeCandidate"], true);

    let duplicate = projects
        .iter()
        .find(|project| project["repoId"] == "repo-duplicate")
        .expect("duplicate project");
    assert_eq!(duplicate["status"], "present_candidate");
    assert!(array_has(
        duplicate,
        "activeBlockers",
        "duplicate_remote_secondary"
    ));

    let research = projects
        .iter()
        .find(|project| project["repoId"] == "repo-research")
        .expect("research project");
    assert_eq!(research["status"], "present_candidate");
    assert!(array_has(research, "classification", "research_checkout"));

    let nested = projects
        .iter()
        .find(|project| project["repoId"] == "repo-nested")
        .expect("nested project");
    assert_eq!(nested["status"], "present_candidate");
    assert!(array_has(nested, "classification", "nested_checkout"));

    let missing_project_json = projects
        .iter()
        .find(|project| project["repoId"] == "repo-missing-project-json")
        .expect("missing project json");
    assert_eq!(missing_project_json["status"], "orphan_candidate");
    assert!(array_has(
        missing_project_json,
        "reasons",
        "missing_project_json"
    ));

    let _ = std::fs::remove_dir_all(checkout_base);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn state_projects_gc_dry_run_and_apply_delete_only_orphans() {
    let root = temp_project_root("state-projects-gc-orphans");
    let state_home = root.join(".asp-home");
    let projects_dir = state_home.join("projects").join("by-id");
    std::fs::create_dir_all(&projects_dir).expect("create projects dir");

    let checkout_base = std::env::current_dir()
        .expect("current dir")
        .join("target")
        .join("state-projects-gc-fixtures")
        .join(root.file_name().expect("fixture root name"));
    let _ = std::fs::remove_dir_all(&checkout_base);
    let active_checkout = checkout_base.join("active-repo");
    let research_checkout = checkout_base.join(".data").join("loopx");
    let no_remote_checkout = checkout_base.join("no-remote");
    for checkout in [&active_checkout, &research_checkout, &no_remote_checkout] {
        std::fs::create_dir_all(checkout).expect("create checkout fixture");
    }

    let write_project =
        |repo_id: &str, checkout_root: &std::path::Path, remote_url: Option<&str>| {
            let project_dir = projects_dir.join(repo_id);
            std::fs::create_dir_all(&project_dir).expect("create project state dir");
            let body = serde_json::json!({
                "stateLayoutVersion": "state-v2",
                "repoId": repo_id,
                "displayName": repo_id,
                "checkoutRoot": checkout_root,
                "gitToplevel": checkout_root,
                "gitDir": checkout_root.join(".git"),
                "gitCommonDir": checkout_root.join(".git"),
                "remoteUrl": remote_url,
                "identityBasis": remote_url.map(|remote| format!("git-remote:{remote}")),
            });
            std::fs::write(
                project_dir.join("project.json"),
                serde_json::to_string_pretty(&body).expect("serialize project json"),
            )
            .expect("write project json");
        };

    let active_dir = projects_dir.join("repo-active");
    let research_dir = projects_dir.join("repo-research");
    let no_remote_dir = projects_dir.join("repo-no-remote");
    let missing_project_json_dir = projects_dir.join("repo-missing-project-json");
    let blocked_registry_dir = projects_dir.join("repo-blocked-registry");
    write_project(
        "repo-active",
        &active_checkout,
        Some("https://github.com/example/active.git"),
    );
    write_project(
        "repo-research",
        &research_checkout,
        Some("https://github.com/example/research.git"),
    );
    write_project("repo-no-remote", &no_remote_checkout, None);
    std::fs::create_dir_all(&missing_project_json_dir)
        .expect("create missing project json fixture");
    std::fs::create_dir_all(
        blocked_registry_dir
            .join("workspaces")
            .join("workspace-test")
            .join("live")
            .join("client")
            .join("agent"),
    )
    .expect("create blocked registry fixture dir");
    std::fs::write(
        blocked_registry_dir
            .join("workspaces")
            .join("workspace-test")
            .join("live")
            .join("client")
            .join("agent")
            .join("session-registry.turso"),
        "registry placeholder",
    )
    .expect("write blocked registry placeholder");

    let dry_run_output = asp_command(&root)
        .env("ASP_STATE_HOME", &state_home)
        .args(["state", "projects", "gc", "--json"])
        .output()
        .expect("run asp state projects gc dry run");
    assert!(
        dry_run_output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&dry_run_output.stderr)
    );
    let dry_run_report: serde_json::Value =
        serde_json::from_slice(&dry_run_output.stdout).expect("parse gc dry run json");
    assert_eq!(dry_run_report["mode"], "dry-run");
    assert_eq!(dry_run_report["summary"]["deleteCandidates"], 4);
    assert_eq!(dry_run_report["summary"]["deleteBlocked"], 0);
    assert_eq!(dry_run_report["summary"]["wouldDelete"], 4);
    assert_eq!(dry_run_report["summary"]["deleted"], 0);
    assert!(active_dir.is_dir(), "active project must survive dry-run");
    assert!(
        research_dir.is_dir(),
        "present project must survive dry-run"
    );
    assert!(
        no_remote_dir.is_dir(),
        "orphan project must survive dry-run"
    );
    assert!(
        missing_project_json_dir.is_dir(),
        "missing-project-json orphan must survive dry-run"
    );
    assert!(
        blocked_registry_dir.is_dir(),
        "registry-blocked orphan must survive dry-run"
    );

    let apply_output = asp_command(&root)
        .env("ASP_STATE_HOME", &state_home)
        .args(["state", "projects", "gc", "--json", "--apply"])
        .output()
        .expect("run asp state projects gc apply");
    assert!(
        apply_output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&apply_output.stderr)
    );
    let apply_report: serde_json::Value =
        serde_json::from_slice(&apply_output.stdout).expect("parse gc apply json");
    assert_eq!(apply_report["mode"], "apply");
    assert_eq!(apply_report["summary"]["deleteCandidates"], 4);
    assert_eq!(apply_report["summary"]["deleteBlocked"], 0);
    assert_eq!(apply_report["summary"]["deleted"], 4);
    assert_eq!(apply_report["summary"]["errors"], 0);
    assert!(active_dir.is_dir(), "active project must survive apply");
    assert!(
        !research_dir.exists(),
        "present but non-active project should be deleted"
    );
    assert!(
        !no_remote_dir.exists(),
        "orphan no-remote project should be deleted"
    );
    assert!(
        !missing_project_json_dir.exists(),
        "orphan missing-project-json project should be deleted"
    );
    assert!(
        !blocked_registry_dir.exists(),
        "orphan with session registry should be deleted when session_registry_present is no longer a delete blocker"
    );

    let audit_after_apply = asp_command(&root)
        .env("ASP_STATE_HOME", &state_home)
        .args(["state", "projects", "audit", "--json"])
        .output()
        .expect("run asp state projects audit after gc");
    assert!(
        audit_after_apply.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&audit_after_apply.stderr)
    );
    let audit_report: serde_json::Value =
        serde_json::from_slice(&audit_after_apply.stdout).expect("parse audit after gc json");
    assert_eq!(audit_report["summary"]["total"], 1);
    assert_eq!(audit_report["summary"]["activeCandidate"], 1);
    assert_eq!(audit_report["summary"]["presentCandidate"], 0);
    assert_eq!(audit_report["summary"]["orphanCandidate"], 0);
    assert_eq!(audit_report["summary"]["deleteBlocked"], 0);

    let _ = std::fs::remove_dir_all(checkout_base);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn state_projects_gc_help_has_no_audit_side_effect() {
    let root = temp_project_root("state-projects-gc-help");
    let state_home = root.join(".asp-home");
    let output = asp_command(&root)
        .env("ASP_STATE_HOME", &state_home)
        .args(["state", "projects", "gc", "--help"])
        .output()
        .expect("run asp state projects gc help");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("asp state projects gc [--json] [--apply]"),
        "stdout={stdout}"
    );
    assert!(
        !stdout.contains("[state-projects-gc]"),
        "help must not run gc, stdout={stdout}"
    );
    assert!(
        !state_home.exists(),
        "help must not create or inspect state home"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn workspace_file_rejection_error_snapshot_and_perf() {
    let root = temp_project_root("search-workspace-file-rejection-api");
    let workspace_file = root.join("build-std.ss");
    std::fs::write(&workspace_file, "(display \"build\")\n").expect("write owner file");
    let workspace = workspace_file.display().to_string();

    let start = Instant::now();
    let error = run_cli_args([
        "gerbil-scheme",
        "search",
        "owner",
        "build-std.ss",
        "items",
        "--query",
        "builded|pended|optimization|make|clan|building",
        "--workspace",
        workspace.as_str(),
        "--view",
        "seeds",
    ])
    .expect_err("file-valued workspace should fail through Rust API");
    let elapsed = start.elapsed();

    assert!(
        elapsed <= WORKSPACE_FILE_REJECTION_API_MAX,
        "workspace file rejection took {elapsed:?}, expected <= {WORKSPACE_FILE_REJECTION_API_MAX:?}"
    );
    let canonical_root = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
    let snapshot = error
        .replace(&canonical_root.display().to_string(), "[ROOT]")
        .replace(&root.display().to_string(), "[ROOT]");
    insta::assert_snapshot!(
        snapshot,
        @r###"--workspace requires a directory project root, got file `[ROOT]/build-std.ss`. Keep the file path as the owner/selector and use a directory workspace, for example `asp gerbil-scheme search owner <file> items --query '<terms>' --workspace . --view seeds`."###
    );

    let _ = std::fs::remove_dir_all(root);
}
