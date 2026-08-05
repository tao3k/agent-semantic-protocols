mod activation_sync;
mod install;

use super::support::{
    asp_command, temp_project_root, write_default_client_hook_config,
    write_root_owned_rust_activation,
};

#[test]
fn cli_doctor_accepts_root_owned_rust_activation() {
    let root = temp_project_root("doctor-activation");
    super::support::write_default_client_hook_config(&root);
    let activation_path = write_root_owned_rust_activation(&root);
    let output = asp_command()
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env("CODEX_HOME", root.join(".codex-home"))
        .args([
            "hook",
            "doctor",
            "--activation",
            activation_path.to_str().expect("utf8 activation path"),
            root.to_str().expect("utf8 project root"),
        ])
        .output()
        .expect("run agent-semantic-protocol doctor");

    let stdout = String::from_utf8(output.stdout).expect("doctor stdout");
    let stderr = String::from_utf8(output.stderr).expect("doctor stderr");
    assert!(
        output.status.success(),
        "doctor failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("[agent-doctor] status=ok")
            || stdout.contains("[agent-doctor] status=warning")
    );
    assert!(stdout.contains("providers=1"));
    assert!(stdout.contains("clientConfigStatus="));
    assert!(stdout.contains("classifierProbe="));
    assert!(stdout.contains("classifierReason="));
    assert!(stdout.contains("enforcement="), "{stdout}");
    assert!(stdout.contains("enforcementProbe="));
    assert!(stdout.contains("enforcementReason="));
    assert!(stdout.contains("|provider language=rust provider=rs-harness"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_doctor_reports_deny_for_codex_exec_command_source_dump() {
    let root = temp_project_root("doctor-classifier-probe");
    let activation_path = write_root_owned_rust_activation(&root);
    write_default_client_hook_config(&root);
    let output = asp_command()
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .args([
            "hook",
            "doctor",
            "--activation",
            activation_path.to_str().expect("utf8 activation path"),
            root.to_str().expect("utf8 project root"),
        ])
        .output()
        .expect("run agent-semantic-protocol doctor");

    let stdout = String::from_utf8(output.stdout).expect("doctor stdout");
    let stderr = String::from_utf8(output.stderr).expect("doctor stderr");
    assert!(
        output.status.success(),
        "doctor failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("[agent-doctor] status=ok")
            || stdout.contains("[agent-doctor] status=warning")
    );
    assert!(stdout.contains("clientConfigStatus=ok"));
    assert!(stdout.contains("binaryContractStatus="));
    assert!(stdout.contains("binaryContractFingerprint=hook-client-v1-"));
    assert!(stdout.contains("activeContractFingerprint="));
    assert!(stdout.contains("classifierProbe=deny"));
    assert!(stdout.contains("classifierReason=bulk-source-dump"));
    assert!(stdout.contains("classifierRule=deny-uncontrolled-source-materialization-commands"));
    assert!(stdout.contains("matchPolicyStatus=partial"), "{stdout}");
    assert!(stdout.contains("matchPolicyRules=18"));
    assert!(stdout.contains("matchPolicyCases=18"));
    let covered = doctor_count(&stdout, "matchPolicyCovered");
    let failures = doctor_count(&stdout, "matchPolicyFailures");
    assert_eq!(covered + failures, 18, "{stdout}");
    assert!(covered > 0, "{stdout}");
    assert!(stdout.contains("enforcement="), "{stdout}");
    assert!(stdout.contains("enforcementProbe="));
    assert!(stdout.contains("enforcementReason="));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

fn doctor_count(output: &str, key: &str) -> usize {
    output
        .split_whitespace()
        .find_map(|field| field.strip_prefix(&format!("{key}=")))
        .unwrap_or_else(|| panic!("doctor output omitted {key}: {output}"))
        .parse()
        .unwrap_or_else(|error| panic!("doctor output has invalid {key}: {error}: {output}"))
}
