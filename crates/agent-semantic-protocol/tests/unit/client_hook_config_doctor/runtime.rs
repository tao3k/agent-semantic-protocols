use super::{
    PROBE_SENTINEL, run_doctor_with_env, stderr, stdout, temp_project_root, write_activation,
    write_client_config, write_codex_project_config, write_executable,
};

#[test]
fn doctor_reports_runtime_profile_health() {
    let root = temp_project_root("doctor-runtime-profiles");
    let activation_path = write_activation(&root);
    write_client_config(&root, "");
    let bin_dir = root.join(".doctor-bin");
    write_executable(&bin_dir, "rs-harness", "#!/bin/sh\nexit 0\n");

    let output = run_doctor_with_env(&root, &activation_path, &[], &[], Some(&bin_dir));

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(!stdout.contains("runtimeProfiles="));
    assert!(stdout.contains("runtimeStatus=available"));
    assert!(stdout.contains("resolvedBinary="));
    assert!(stdout.contains("/rs-harness"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn doctor_reports_enforced_when_codex_probe_observes_deny() {
    let root = temp_project_root("doctor-codex-probe-deny");
    let activation_path = write_activation(&root);
    write_codex_project_config(&root);
    write_client_config(
        &root,
        r#"
[[rules]]
id = "valid-doctor-rule"
decision = "deny"
"#,
    );
    let bin_dir = root.join(".test-bin");
    let codex = write_executable(
        &bin_dir,
        "codex",
        "#!/bin/sh\nprintf '%s\\n' '{\"permissionDecision\":\"deny\",\"permissionDecisionReason\":\"direct-source-read\"}'\n",
    );
    write_executable(&bin_dir, "asp", "#!/bin/sh\nexit 0\n");

    let output = run_doctor_with_env(
        &root,
        &activation_path,
        &[("ASP_CODEX_CLI_ENFORCEMENT_PROBE", "1")],
        &[("ASP_CODEX_CLI", codex.to_str().expect("utf8 codex path"))],
        Some(&bin_dir),
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains("enforcement=enforced"));
    assert!(stdout.contains("enforcementReason=hook-deny-observed"));
    assert!(stdout.contains("|enforcement status=enforced"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn doctor_reports_unproven_when_codex_exec_has_no_hook_surface() {
    let root = temp_project_root("doctor-codex-probe-leak");
    let activation_path = write_activation(&root);
    write_codex_project_config(&root);
    write_client_config(
        &root,
        r#"
[[rules]]
id = "valid-doctor-rule"
decision = "deny"
"#,
    );
    let bin_dir = root.join(".test-bin");
    let codex = write_executable(
        &bin_dir,
        "codex",
        &format!("#!/bin/sh\nprintf '%s\\n' '{PROBE_SENTINEL}'\n"),
    );
    write_executable(&bin_dir, "asp", "#!/bin/sh\nexit 0\n");

    let output = run_doctor_with_env(
        &root,
        &activation_path,
        &[("ASP_CODEX_CLI_ENFORCEMENT_PROBE", "1")],
        &[("ASP_CODEX_CLI", codex.to_str().expect("utf8 codex path"))],
        Some(&bin_dir),
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains("enforcement=unproven"));
    assert!(stdout.contains("enforcementProbe=codex-exec-unsupported"));
    assert!(stdout.contains("enforcementReason=codex-exec-hook-surface-missing"));
    assert!(stdout.contains("|enforcement status=unproven"));
    assert!(stdout.contains("sentinel=true"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn doctor_reports_configured_but_not_enforced_when_hook_event_leaks_source() {
    let root = temp_project_root("doctor-codex-probe-hook-leak");
    let activation_path = write_activation(&root);
    write_codex_project_config(&root);
    write_client_config(
        &root,
        r#"
[[rules]]
id = "valid-doctor-rule"
decision = "deny"
"#,
    );
    let bin_dir = root.join(".test-bin");
    let codex = write_executable(
        &bin_dir,
        "codex",
        &format!("#!/bin/sh\nprintf '%s\\n' 'HookStarted {PROBE_SENTINEL}'\n"),
    );
    write_executable(&bin_dir, "asp", "#!/bin/sh\nexit 0\n");

    let output = run_doctor_with_env(
        &root,
        &activation_path,
        &[("ASP_CODEX_CLI_ENFORCEMENT_PROBE", "1")],
        &[("ASP_CODEX_CLI", codex.to_str().expect("utf8 codex path"))],
        Some(&bin_dir),
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains("enforcement=configured-but-not-enforced"));
    assert!(stdout.contains("enforcementReason=source-sentinel-leaked"));
    assert!(stdout.contains("|enforcement status=configured-but-not-enforced"));
    assert!(stdout.contains("sentinel=true"));
    assert!(stdout.contains("hookEvent=true"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}
