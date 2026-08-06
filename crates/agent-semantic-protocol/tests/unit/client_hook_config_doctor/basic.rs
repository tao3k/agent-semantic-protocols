use super::{
    run_doctor, run_doctor_with_env, stderr, stdout, temp_project_root, write_activation,
    write_client_config, write_codex_global_inline_config, write_codex_project_plugin_config,
    write_executable,
};

#[test]
fn doctor_rejects_missing_matcher_config() {
    let root = temp_project_root("doctor-missing-config");
    let activation_path = write_activation(&root);

    let output = run_doctor(&root, &activation_path);

    assert!(!output.status.success());
    let stderr = stderr(&output);
    assert!(stderr.contains("invalid effective client hook config"));
    assert!(stderr.contains(".agent-semantic-protocols/hooks/config.toml"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn doctor_reports_valid_client_hook_config() {
    let root = temp_project_root("doctor-valid-config");
    let activation_path = write_activation(&root);
    write_client_config(
        &root,
        r#"
[[rules]]
id = "valid-doctor-rule"
decision = "deny"
[rules.match]
tool = "Bash"
"#,
    );

    let output = run_doctor(&root, &activation_path);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains("clientConfigStatus=ok"));
    assert!(stdout.contains("configContractStatus=missing"));
    assert!(stdout.contains("hookShellMode=login"));
    assert!(stdout.contains("hookShellBinaryStatus="));
    assert!(stdout.contains("hookShellBinaryPath="));
    assert!(stdout.contains("eventState=missing"));
    assert!(stdout.contains("eventStateBytes=0"));
    assert!(stdout.contains("eventStateAgeMs=unavailable"));
    assert!(stdout.contains("enforcement=unavailable"));
    assert!(stdout.contains("enforcementReason=project-hook-missing"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn doctor_reports_live_hook_event_state() {
    let root = temp_project_root("doctor-live-event-state");
    let activation_path = write_activation(&root);
    write_client_config(
        &root,
        r#"
[[rules]]
id = "live-event-state-rule"
decision = "deny"
"#,
    );
    std::fs::write(
        activation_path
            .parent()
            .expect("activation state parent")
            .join("events.jsonl"),
        b"{}\n",
    )
    .expect("write hook event state");

    let output = run_doctor(&root, &activation_path);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains("eventState=live"), "{stdout}");
    assert!(stdout.contains("eventStateBytes=3"), "{stdout}");
    assert!(!stdout.contains("eventStateAgeMs=unavailable"), "{stdout}");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn strict_doctor_rejects_config_without_contract_fingerprint() {
    let root = temp_project_root("doctor-strict-contract");
    let activation_path = write_activation(&root);
    write_client_config(
        &root,
        r#"
[[rules]]
id = "valid-but-stale-doctor-rule"
decision = "deny"
"#,
    );

    let output = super::run_doctor_strict(&root, &activation_path);

    assert!(!output.status.success());
    let stderr = stderr(&output);
    assert!(
        stderr.contains("hook contract freshness gate failed: config=missing"),
        "{stderr}"
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn doctor_reports_native_inline_debug_mode() {
    let root = temp_project_root("doctor-project-plugin-hook-present");
    let activation_path = write_activation(&root);
    write_codex_project_plugin_config(&root);
    write_client_config(
        &root,
        r#"
[[rules]]
id = "valid-doctor-rule"
decision = "deny"
[rules.match]
tool = "Bash"
"#,
    );
    let bin_dir = root.join(".test-bin");
    let asp_binary = write_executable(&bin_dir, "asp", "#!/bin/sh\nexit 0\n");
    write_codex_global_inline_config(&root, &asp_binary);

    let output = run_doctor_with_env(&root, &activation_path, &[], &[], Some(&bin_dir));

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains("hook=true"), "{stdout}");
    assert!(
        stdout.contains("hookMode=codex-native-inline-debug"),
        "{stdout}"
    );
    assert!(stdout.contains("pluginHook=false"), "{stdout}");
    assert!(stdout.contains("trust=true"), "{stdout}");
    assert!(stdout.contains("projectTrust=false"), "{stdout}");
    assert!(stdout.contains("hookStateTrust=true"), "{stdout}");
    assert!(stdout.contains("trustMissing=0"), "{stdout}");
    assert!(stdout.contains("enforcement=unproven"), "{stdout}");
    assert!(
        stdout.contains("backgroundThreadHook=host-surface-unproven"),
        "{stdout}"
    );
    assert!(
        stdout.contains("hostSurface=codex_app.create_thread"),
        "{stdout}"
    );
    assert!(
        stdout.contains("verificationHint=native-thread-required"),
        "{stdout}"
    );
    assert!(
        stdout.contains("enforcementReason=codex-exec-probe-disabled"),
        "{stdout}"
    );
    assert!(!stdout.contains("project-hook-missing"), "{stdout}");
    assert!(stdout.contains("|trust project=untrusted"), "{stdout}");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn codex_global_inline_hooks_use_stable_public_surface_and_bounded_timeout() {
    let rendered = agent_semantic_hook::codex_global_hook_block_with_binary(Some(
        std::path::Path::new("/state/runtime/bin/asp"),
    ));
    let hooks: toml::Value = toml::from_str(&rendered).expect("parse inline Hook config");
    let hooks = hooks["hooks"].as_table().expect("hooks table");

    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PermissionRequest",
        "PostToolUse",
        "SubagentStart",
        "SubagentStop",
        "Stop",
    ] {
        let command_hook = hooks[event][0]["hooks"][0]
            .as_table()
            .unwrap_or_else(|| panic!("{event} command hook object"));
        let command = command_hook["command"]
            .as_str()
            .unwrap_or_else(|| panic!("{event} command string"));
        assert!(
            command.contains("exec '/state/runtime/bin/asp' hook "),
            "{event} command must use the stable public hook surface: {command}"
        );
        assert!(
            !command.contains("direnv exec"),
            "{event} command must not wrap through project direnv: {command}"
        );
        assert_eq!(
            command_hook["timeout"].as_integer(),
            Some(1),
            "{event} hook timeout must stay bounded"
        );
    }
}

#[test]
fn doctor_rejects_invalid_client_hook_config() {
    let root = temp_project_root("doctor-invalid-config");
    let activation_path = write_activation(&root);
    write_client_config(&root, "schemaId = 7");

    let output = run_doctor(&root, &activation_path);

    assert!(!output.status.success());
    let stderr = stderr(&output);
    assert!(
        stderr.contains("invalid effective client hook config"),
        "{stderr}"
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn doctor_rejects_duplicate_client_hook_rule_ids() {
    let root = temp_project_root("doctor-duplicate-config-rule");
    let activation_path = write_activation(&root);
    write_client_config(
        &root,
        r#"
[[rules]]
id = "duplicate-rule"
decision = "deny"

[[rules]]
id = "duplicate-rule"
decision = "deny"
"#,
    );
    let output = run_doctor(&root, &activation_path);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("duplicate client hook rule id"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}
