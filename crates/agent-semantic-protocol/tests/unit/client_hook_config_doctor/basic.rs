use super::{
    run_doctor, run_doctor_with_env, stderr, stdout, temp_project_root, write_activation,
    write_client_config, write_codex_project_plugin_config, write_executable,
};

#[test]
fn doctor_uses_default_client_hook_config_when_override_is_absent() {
    let root = temp_project_root("doctor-missing-config");
    let activation_path = write_activation(&root);

    let output = run_doctor(&root, &activation_path);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains("clientConfig="));
    assert!(stdout.contains(".agent-semantic-protocols/hooks/config.toml"));
    assert!(stdout.contains("clientConfigStatus=default"));
    assert!(stdout.contains("configContractStatus=match"));
    assert!(stdout.contains("configuredContractFingerprint=hook-client-v1-"));
    assert!(stdout.contains("classifierProbe=deny"));
    assert!(!stdout.contains("client-config-missing"));
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
    assert!(stdout.contains("enforcement=unavailable"));
    assert!(stdout.contains("enforcementReason=project-hook-missing"));
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
fn doctor_treats_project_plugin_hooks_as_hook_present() {
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
    write_executable(&bin_dir, "asp", "#!/bin/sh\nexit 0\n");

    let output = run_doctor_with_env(&root, &activation_path, &[], &[], Some(&bin_dir));

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains("hook=true"), "{stdout}");
    assert!(stdout.contains("hookMode=codex-plugin"), "{stdout}");
    assert!(stdout.contains("pluginHook=true"), "{stdout}");
    assert!(stdout.contains("trust=false"), "{stdout}");
    assert!(stdout.contains("projectTrust=false"), "{stdout}");
    assert!(stdout.contains("hookStateTrust=false"), "{stdout}");
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
fn codex_plugin_hooks_use_global_asp_and_bounded_timeout() {
    let hooks: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../asp-codex-plugin/hooks/hooks.json"
    ))
    .expect("parse hooks.json");
    let hooks = hooks["hooks"].as_object().expect("hooks object");

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
            .as_object()
            .unwrap_or_else(|| panic!("{event} command hook object"));
        let command = command_hook["command"]
            .as_str()
            .unwrap_or_else(|| panic!("{event} command string"));
        assert!(
            command.starts_with("asp hook "),
            "{event} command must use global asp: {command}"
        );
        assert!(
            !command.contains("direnv exec"),
            "{event} command must not wrap through project direnv: {command}"
        );
        assert_eq!(
            command_hook["timeout"].as_i64(),
            Some(5),
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
