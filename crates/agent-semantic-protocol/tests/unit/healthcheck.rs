use agent_semantic_hook::{builtin_provider_manifests, provider_manifest_digest};
use serde_json::{Value, json};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn healthcheck_reports_git_cache_agents_and_activation_runtime() {
    let root = prepared_project("healthcheck-compact");
    let provider = write_executable(&root, "rs-harness");
    write_activation(&root, &provider);

    let output = run_healthcheck(&root, &["."], &[]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    let git_toplevel = root.canonicalize().expect("canonical root");
    assert!(stdout.contains("[asp-healthcheck] status="));
    assert!(stdout.contains(&format!("gitToplevel={}", git_toplevel.display())));
    assert!(stdout.contains("cacheSource=git-toplevel"));
    assert!(stdout.contains("|env PRJ_CACHE_HOME=unset"));
    assert!(stdout.contains("|path agentsSkill="));
    assert!(stdout.contains("status=ok"));
    assert!(stdout.contains("|path activation="));
    assert!(stdout.contains("providers=1"));
    assert!(stdout.contains("|activationRuntime status=ok providers=1"));
    assert!(stdout.contains("|provider language=rust provider=rs-harness runtime=available"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn healthcheck_json_reports_project_runtime_layout() {
    let root = prepared_project("healthcheck-json");
    let provider = write_executable(&root, "rs-harness");
    write_activation(&root, &provider);

    let output = run_healthcheck(&root, &["--json", "."], &[]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let value: Value = serde_json::from_str(&stdout(&output)).expect("parse healthcheck JSON");
    assert_eq!(
        value["schemaId"],
        json!("agent.semantic-protocols.healthcheck")
    );
    assert_eq!(value["cacheSource"], json!("git-toplevel"));
    assert_eq!(value["paths"]["activation"]["status"], json!("ok"));
    assert_eq!(value["activationRuntime"]["providerCount"], json!(1));
    assert_eq!(value["providers"][0]["languageId"], json!("rust"));
    assert_eq!(value["env"]["PRJ_CACHE_HOME"], Value::Null);
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn healthcheck_prefers_git_toplevel_over_prj_cache_home_when_set() {
    let root = prepared_project("healthcheck-prj-cache-home");
    let cache_home = root.join(".cache");

    let output = run_healthcheck(
        &root,
        &["."],
        &[(
            "PRJ_CACHE_HOME",
            cache_home.to_str().expect("utf8 temp path"),
        )],
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains("cacheSource=git-toplevel"));
    assert!(stdout.contains("PRJ_CACHE_HOME=set:"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn healthcheck_uses_prj_cache_home_outside_git_worktree() {
    let root = temp_project_root("healthcheck-prj-cache-home-no-git");
    let cache_home = root.join("cache-home");

    let output = run_healthcheck(
        &root,
        &["."],
        &[(
            "PRJ_CACHE_HOME",
            cache_home.to_str().expect("utf8 temp path"),
        )],
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains("[asp-healthcheck] status=error"));
    assert!(stdout.contains("cacheSource=prj-cache-home"));
    assert!(stdout.contains("PRJ_CACHE_HOME=set:"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

fn prepared_project(name: &str) -> PathBuf {
    let root = temp_project_root(name);
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    let skill_dir = root.join(".agents/skills/agent-semantic-protocols");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(skill_dir.join("SKILL.md"), "# test skill\n").expect("write skill");
    root
}

fn write_activation(root: &Path, provider: &Path) {
    let manifest = rust_manifest();
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let activation_dir = root.join(".cache/agent-semantic-protocol/hooks");
    std::fs::create_dir_all(&activation_dir).expect("create activation dir");
    let activation = json!({
        "schemaId": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_VERSION,
        "protocolId": agent_semantic_hook::HOOK_PROTOCOL_ID,
        "protocolVersion": agent_semantic_hook::HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "generatedBy": { "runtime": "asp", "version": "test" },
        "providers": [{
            "manifestId": manifest.manifest_id,
            "manifestDigest": manifest_digest,
            "languageId": manifest.language_id,
            "providerId": manifest.provider_id,
            "binary": manifest.binary,
            "providerCommandPrefix": [provider.display().to_string()],
            "coverage": {
                "packageRoots": ["."],
                "sourceRoots": manifest.source.default_source_roots,
                "configFiles": manifest.source.default_config_files,
                "sourceExtensions": manifest.source.default_extensions,
                "ignoredPathPrefixes": manifest.source.default_ignored_path_prefixes
            }
        }]
    });
    std::fs::write(
        activation_dir.join("activation.json"),
        serde_json::to_string_pretty(&activation).expect("serialize activation"),
    )
    .expect("write activation");
}

fn rust_manifest() -> agent_semantic_hook::ProviderManifest {
    builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "rust")
        .expect("rust manifest")
}

fn run_healthcheck(root: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command.current_dir(root).arg("healthcheck").args(args);
    command.env_remove("PRJ_CACHE_HOME");
    for (key, value) in envs {
        command.env(key, value);
    }
    command.output().expect("run asp healthcheck")
}

fn write_executable(root: &Path, name: &str) -> PathBuf {
    let bin_dir = root.join(".test-bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let path = bin_dir.join(name);
    std::fs::write(&path, "#!/bin/sh\nexit 0\n").expect("write executable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("chmod executable");
    }
    path
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn temp_project_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = env::temp_dir().join(format!("agent-semantic-protocol-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp project root");
    root
}
