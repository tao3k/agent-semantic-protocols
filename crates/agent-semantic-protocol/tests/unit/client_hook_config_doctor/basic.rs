use super::{run_doctor, stderr, stdout, temp_project_root, write_activation, write_client_config};

#[test]
fn doctor_reports_missing_client_hook_config() {
    let root = temp_project_root("doctor-missing-config");
    let activation_path = write_activation(&root);

    let output = run_doctor(&root, &activation_path);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains("clientConfig=.codex/agent-semantic-protocol/hooks/config.toml"));
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
