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
    let activation_path = canonical_activation_path(&root);
    assert!(stdout.contains("[asp-healthcheck] status="));
    assert!(stdout.contains(&format!("gitToplevel={}", git_toplevel.display())));
    assert!(stdout.contains("cacheSource=git-toplevel"));
    assert!(stdout.contains("|env PRJ_CACHE_HOME=unset"));
    assert!(stdout.contains("|path agentsSkill="));
    assert!(stdout.contains("status=ok"));
    assert!(stdout.contains(&format!("|path activation={}", activation_path.display())));
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

    let output = run_healthcheck(&root, &["--json", "."], &[("ASP_NO_AGENT_PLATFORM", "1")]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let value: Value = serde_json::from_str(&stdout(&output)).expect("parse healthcheck JSON");
    assert_eq!(
        value["schemaId"],
        json!("agent.semantic-protocols.healthcheck")
    );
    assert_eq!(value["cacheSource"], json!("git-toplevel"));
    assert_eq!(value["paths"]["activation"]["status"], json!("ok"));
    assert_eq!(
        value["paths"]["activation"]["path"],
        json!(canonical_activation_path(&root).display().to_string())
    );
    assert_eq!(value["activationRuntime"]["providerCount"], json!(1));
    assert_eq!(value["providers"][0]["languageId"], json!("rust"));
    assert_eq!(value["env"]["PRJ_CACHE_HOME"], Value::Null);
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn healthcheck_accepts_enabled_global_codex_plugin_skill() {
    let root = temp_project_root("healthcheck-global-plugin-skill");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    std::fs::create_dir_all(root.join(".agents")).expect("create agents dir");
    let provider = write_executable(&root, "rs-harness");
    write_activation(&root, &provider);

    let codex_home = root.join("codex-home");
    std::fs::create_dir_all(&codex_home).expect("create Codex home");
    std::fs::write(
        codex_home.join("config.toml"),
        "[plugins.\"asp-codex-plugin@asp-project\"]\nenabled = true\n",
    )
    .expect("write Codex config");
    let plugin_skill = codex_home.join(
        "plugins/cache/asp-project/asp-codex-plugin/0.1.0/skills/agent-semantic-protocols/SKILL.org",
    );
    std::fs::create_dir_all(plugin_skill.parent().expect("plugin skill parent"))
        .expect("create plugin skill parent");
    std::fs::write(&plugin_skill, "#+TITLE: plugin skill\n").expect("write plugin skill");

    let output = run_healthcheck(
        &root,
        &["."],
        &[("CODEX_HOME", codex_home.to_str().expect("utf8 Codex home"))],
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains(&format!(
        "|path pluginSkill={} status=ok",
        plugin_skill.display()
    )));
    assert!(!stdout.contains("warn code=missing-agent-skill"));
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
    assert!(stdout.contains(&format!(
        "|path activation={}",
        canonical_activation_path(&root).display()
    )));
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
    assert!(stdout.contains(&format!(
        "|path activation={}",
        canonical_activation_path(&root).display()
    )));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

fn prepared_project(name: &str) -> PathBuf {
    let root = temp_project_root(name);
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    let skill_dir = root.join(".agents/skills/agent-semantic-protocols");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(skill_dir.join("SKILL.org"), "#+TITLE: test skill\n").expect("write skill");
    root
}

fn write_activation(root: &Path, provider: &Path) {
    let manifest = rust_manifest();
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let activation_path = canonical_activation_path(root);
    let activation_dir = activation_path.parent().expect("activation parent");
    std::fs::create_dir_all(&activation_dir).expect("create activation dir");
    let activation = json!({
        "schemaId": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_VERSION,
        "protocolId": agent_semantic_hook::HOOK_PROTOCOL_ID,
        "protocolVersion": agent_semantic_hook::HOOK_PROTOCOL_VERSION,
        "projectRoot": root.canonicalize().expect("canonical root").display().to_string(),
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
        activation_path,
        serde_json::to_string_pretty(&activation).expect("serialize activation"),
    )
    .expect("write activation");
}

fn canonical_activation_path(root: &Path) -> PathBuf {
    let context =
        agent_semantic_client_core::ProjectContext::resolve(root).expect("resolve project context");
    agent_semantic_runtime::project_state_paths(context.cwd())
        .expect("resolve project state paths")
        .activation_path
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
