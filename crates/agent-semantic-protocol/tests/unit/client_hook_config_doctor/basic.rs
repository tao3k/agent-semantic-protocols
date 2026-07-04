use super::{
    run_doctor, run_doctor_with_env, stderr, stdout, temp_project_root, write_activation,
    write_client_config, write_codex_home_project_trust, write_codex_project_plugin_config,
    write_executable,
};

#[test]
fn doctor_reports_missing_client_hook_config() {
    let root = temp_project_root("doctor-missing-config");
    let activation_path = write_activation(&root);

    let output = run_doctor(&root, &activation_path);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains("clientConfig="));
    assert!(stdout.contains(".agent-semantic-protocols/hooks/config.toml"));
    assert!(stdout.contains("clientConfigStatus=missing"));
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
    assert!(stdout.contains("enforcement=unavailable"));
    assert!(stdout.contains("enforcementReason=project-hook-missing"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn doctor_treats_project_plugin_hooks_as_hook_present() {
    let root = temp_project_root("doctor-project-plugin-hook-present");
    let activation_path = write_activation(&root);
    write_codex_project_plugin_config(&root);
    write_codex_home_project_trust(&root);
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
    assert!(stdout.contains("trust=true"), "{stdout}");
    assert!(stdout.contains("projectTrust=true"), "{stdout}");
    assert!(stdout.contains("hookStateTrust=true"), "{stdout}");
    assert!(stdout.contains("trustMissing=0"), "{stdout}");
    assert!(stdout.contains("enforcement=unproven"), "{stdout}");
    assert!(
        stdout.contains("enforcementReason=codex-exec-probe-disabled"),
        "{stdout}"
    );
    assert!(!stdout.contains("project-hook-missing"), "{stdout}");
    assert!(!stdout.contains("|trust missing="), "{stdout}");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn doctor_rejects_invalid_client_hook_config() {
    let root = temp_project_root("doctor-invalid-config");
    let activation_path = write_activation(&root);
    write_client_config(&root, "schemaId = 7");

    let output = run_doctor(&root, &activation_path);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("invalid client hook config"));
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
