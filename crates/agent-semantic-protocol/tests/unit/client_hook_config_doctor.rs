use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_hook::{
    builtin_provider_manifests, codex_hook_block, merge_codex_config, provider_manifest_digest,
};
use serde_json::json;

const PROBE_SENTINEL: &str = "ASP_CODEX_HOOK_ENFORCEMENT_PROBE_SENTINEL_DO_NOT_LEAK";

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
    assert!(stdout.contains("enforcement=not-run"));
    assert!(stdout.contains("enforcementReason=probe-disabled"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn doctor_reports_runtime_profile_health() {
    let root = temp_project_root("doctor-runtime-profiles");
    let activation_path = write_activation(&root);
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
fn doctor_reports_configured_but_not_enforced_when_codex_probe_leaks_source() {
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
    assert!(stdout.contains("enforcement=configured-but-not-enforced"));
    assert!(stdout.contains("enforcementReason=source-sentinel-leaked"));
    assert!(stdout.contains("|enforcement status=configured-but-not-enforced"));
    assert!(stdout.contains("sentinel=true"));
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

fn write_client_config(root: &std::path::Path, content: &str) {
    let config_path = root.join(".codex/agent-semantic-protocol/hooks/config.toml");
    std::fs::create_dir_all(config_path.parent().expect("config parent"))
        .expect("create config dir");
    std::fs::write(config_path, content).expect("write client config");
}

fn write_codex_project_config(root: &std::path::Path) {
    let config_path = root.join(".codex/config.toml");
    std::fs::create_dir_all(config_path.parent().expect("project config parent"))
        .expect("create project config dir");
    std::fs::write(config_path, merge_codex_config("", &codex_hook_block()))
        .expect("write project Codex config");
}

fn write_activation(root: &std::path::Path) -> PathBuf {
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
    activation_path
}

fn run_doctor(root: &std::path::Path, activation_path: &std::path::Path) -> std::process::Output {
    run_doctor_with_env(root, activation_path, &[], &[], None)
}

fn run_doctor_with_env(
    root: &std::path::Path,
    activation_path: &std::path::Path,
    envs: &[(&str, &str)],
    env_paths: &[(&str, &str)],
    path_prefix: Option<&std::path::Path>,
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command.current_dir(root).args([
        "hook",
        "doctor",
        "--client",
        "codex",
        "--activation",
        activation_path.to_str().expect("utf8 activation path"),
        ".",
    ]);
    for (key, value) in envs {
        command.env(key, value);
    }
    for (key, value) in env_paths {
        command.env(key, value);
    }
    if let Some(path_prefix) = path_prefix {
        command.env("PATH", prepend_path(path_prefix));
    }
    command.env_remove("PRJ_CACHE_HOME");
    command.output().expect("run asp hook doctor")
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn temp_project_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-hook-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp project root");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    root
}

fn write_executable(root: &std::path::Path, name: &str, content: &str) -> PathBuf {
    std::fs::create_dir_all(root).expect("create executable dir");
    let path = root.join(name);
    std::fs::write(&path, content).expect("write executable");
    make_executable(&path);
    path
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .expect("executable metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("chmod executable");
}

#[cfg(not(unix))]
fn make_executable(_path: &std::path::Path) {}

fn prepend_path(first: &std::path::Path) -> std::ffi::OsString {
    let mut paths = vec![first.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    std::env::join_paths(paths).expect("join PATH")
}

fn root_owned_rust_activation_json() -> String {
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "rust")
        .expect("rust manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("digest manifest");
    serde_json::to_string_pretty(&json!({
        "schemaId": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_VERSION,
        "protocolId": agent_semantic_hook::HOOK_PROTOCOL_ID,
        "protocolVersion": agent_semantic_hook::HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "generatedBy": {"runtime": "agent-semantic-hook", "version": "test"},
        "providers": [{
            "manifestId": manifest.manifest_id,
            "manifestDigest": manifest_digest,
            "languageId": manifest.language_id,
            "providerId": manifest.provider_id,
            "binary": manifest.binary,
            "providerCommandPrefix": [],
            "coverage": {
                "packageRoots": ["."],
                "sourceRoots": ["src", "tests", "crates", "examples", "benches"],
                "configFiles": ["Cargo.toml", "Cargo.lock"],
                "sourceExtensions": [".rs"],
                "ignoredPathPrefixes": [
                    ".cache",
                    ".direnv",
                    ".git",
                    ".idea",
                    ".jj",
                    ".run",
                    ".vscode",
                    "node_modules",
                    "target",
                    ".codex/harness-state",
                    ".codex/rs-harness"
                ]
            }
        }]
    }))
    .expect("serialize root-owned rust activation")
}
