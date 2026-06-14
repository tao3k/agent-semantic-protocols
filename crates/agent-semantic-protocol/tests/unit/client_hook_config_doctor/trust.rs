use super::{
    run_doctor, stderr, stdout, temp_project_root, write_activation, write_client_config,
    write_codex_project_config, write_stale_codex_home_config,
};

#[test]
fn doctor_reports_codex_project_not_trusted() {
    let root = temp_project_root("doctor-codex-project-not-trusted");
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

    let output = run_doctor(&root, &activation_path);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains("trust=false"));
    assert!(stdout.contains("projectTrust=false"));
    assert!(stdout.contains("hookStateTrust=false"));
    assert!(stdout.contains("trustMissing=8"));
    assert!(stdout.contains("|codex-app projectConfig=.codex/config.toml"));
    assert!(stdout.contains("|trust project=untrusted reason=project-not-trusted"));
    assert!(stdout.contains("|trust missing="));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn doctor_reports_stale_codex_hook_state() {
    let root = temp_project_root("doctor-codex-stale-hook-state");
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
    write_stale_codex_home_config(&root);

    let output = run_doctor(&root, &activation_path);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains("trust=false"));
    assert!(stdout.contains("projectTrust=true"));
    assert!(stdout.contains("hookStateTrust=false"));
    assert!(stdout.contains("trustMissing=7"));
    assert!(stdout.contains("trustStale=1"));
    assert!(stdout.contains("|trust stale=pre-tool"));
    assert!(stdout.contains("|trust missing="));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}
