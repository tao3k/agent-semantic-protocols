use agent_semantic_hook::{builtin_provider_manifests, provider_manifest_digest};
use serde_json::{Value, json};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn healthcheck_reports_state_core_agents_and_activation_runtime() {
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
    assert!(stdout.contains(" stateHome="));
    assert!(stdout.contains(" repoId=repo-"));
    assert!(stdout.contains(" workspaceId=workspace-"));
    let plugin_skill = root.join(
        "codex-home/plugins/cache/asp-project/asp-codex-plugin/0.1.0/skills/agent-semantic-protocols/SKILL.org",
    );
    assert!(stdout.contains(&format!(
        "|skill authority=plugin-installed path={} status=ok error=none",
        plugin_skill.display()
    )));
    assert!(!stdout.contains("agentsSkill="));
    assert!(!stdout.contains("pluginSkill="));
    assert!(stdout.contains(&format!("|path activation={}", activation_path.display())));
    assert!(stdout.contains("providers=1"));
    assert!(stdout.contains("|activationRuntime status=ok providers=1"));
    assert!(stdout.contains("|provider language=rust provider=rs-harness runtime=available"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn healthcheck_json_reports_state_core_layout() {
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
    assert!(value["stateHome"].as_str().is_some());
    assert!(
        value["repoId"]
            .as_str()
            .is_some_and(|value| value.starts_with("repo-"))
    );
    assert!(
        value["workspaceId"]
            .as_str()
            .is_some_and(|value| value.starts_with("workspace-"))
    );
    assert_eq!(value["paths"]["activation"]["status"], json!("ok"));
    assert_eq!(
        value["paths"]["activation"]["path"],
        json!(canonical_activation_path(&root).display().to_string())
    );
    assert_eq!(value["activationRuntime"]["providerCount"], json!(1));
    assert_eq!(value["providers"][0]["languageId"], json!("rust"));
    assert_eq!(value["skill"]["authority"], json!("plugin-installed"));
    assert_eq!(value["skill"]["status"], json!("ok"));
    assert_eq!(value["skill"]["error"], Value::Null);
    assert!(value["paths"].get("agentsSkill").is_none());
    assert!(value["paths"].get("pluginSkill").is_none());
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
        "|skill authority=plugin-installed path={} status=ok error=none",
        plugin_skill.display()
    )));
    assert!(!stdout.contains("warn code=missing-agent-skill"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn healthcheck_rejects_stray_project_skill_when_plugin_skill_is_missing() {
    let root = temp_project_root("healthcheck-stray-project-skill");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    let project_skill = root.join(".agents/skills/agent-semantic-protocols/SKILL.org");
    std::fs::create_dir_all(project_skill.parent().expect("project skill parent"))
        .expect("create project skill parent");
    std::fs::write(project_skill, "#+TITLE: stray checkout skill\n")
        .expect("write stray project skill");
    let provider = write_executable(&root, "rs-harness");
    write_activation(&root, &provider);
    let codex_home = root.join("codex-home");
    std::fs::create_dir_all(&codex_home).expect("create Codex home");
    std::fs::write(
        codex_home.join("config.toml"),
        "[plugins.\"asp-codex-plugin@asp-project\"]\nenabled = true\n",
    )
    .expect("write enabled Codex plugin config");

    let output = run_healthcheck(&root, &["."], &[]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(
        stdout.contains(
            "|skill authority=plugin-installed path=missing status=unresolved error=none"
        ),
        "{stdout}"
    );
    assert!(stdout.contains("error code=invalid-agent-skill"));
    assert!(!stdout.contains("agentsSkill="));
    assert!(!stdout.contains("pluginSkill="));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn healthcheck_json_reports_plugin_skill_resolver_error() {
    let root = temp_project_root("healthcheck-plugin-skill-resolver-error");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    std::fs::create_dir_all(root.join(".agents")).expect("create agents dir");
    let provider = write_executable(&root, "rs-harness");
    write_activation(&root, &provider);

    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command
        .current_dir(&root)
        .arg("healthcheck")
        .args(["--json", "."]);
    command.env_remove("CODEX_HOME");
    command.env_remove("HOME");
    command.env("ASP_STATE_HOME", root.join("state-home"));
    command.env("ASP_NO_AGENT_PLATFORM", "1");
    let output = command
        .output()
        .expect("run asp healthcheck without Codex home");

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let value: Value = serde_json::from_str(&stdout(&output)).expect("parse healthcheck JSON");
    assert_eq!(value["skill"]["authority"], json!("plugin-installed"));
    assert_eq!(value["skill"]["path"], Value::Null);
    assert_eq!(value["skill"]["status"], json!("invalid"));
    assert_eq!(
        value["skill"]["error"],
        json!("missing CODEX_HOME and HOME; cannot locate Codex config")
    );
    assert!(value["issues"].as_array().is_some_and(|issues| {
        issues.iter().any(|issue| {
            issue["code"] == json!("invalid-agent-skill")
                && issue["message"]
                    == json!("missing CODEX_HOME and HOME; cannot locate Codex config")
        })
    }));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

fn prepared_project(name: &str) -> PathBuf {
    let root = temp_project_root(name);
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    let skill_dir = root.join(".agents/skills/agent-semantic-protocols");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(skill_dir.join("SKILL.org"), "#+TITLE: test skill\n").expect("write skill");
    let codex_home = root.join("codex-home");
    std::fs::create_dir_all(&codex_home).expect("create Codex home");
    std::fs::write(
        codex_home.join("config.toml"),
        "[plugins.\"asp-codex-plugin@asp-project\"]\nenabled = true\n",
    )
    .expect("write enabled Codex plugin config");
    let plugin_skill = codex_home.join(
        "plugins/cache/asp-project/asp-codex-plugin/0.1.0/skills/agent-semantic-protocols/SKILL.org",
    );
    std::fs::create_dir_all(plugin_skill.parent().expect("plugin skill parent"))
        .expect("create plugin skill parent");
    std::fs::write(plugin_skill, "#+TITLE: plugin skill\n").expect("write plugin skill");
    root
}

fn write_activation(root: &Path, provider: &Path) {
    let manifest = rust_manifest();
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let provider_command_prefix = vec![provider.display().to_string()];
    let provider_artifact_digest =
        agent_semantic_content_identity::file_content_digest_v1(provider)
            .expect("digest provider executable artifact");
    let execution_command_digest = agent_semantic_hook::provider_execution_command_digest(
        &provider_command_prefix,
        &provider_artifact_digest,
    )
    .expect("provider execution command digest");
    let routes =
        agent_semantic_hook::materialize_provider_routes(&manifest).expect("provider routes");
    let activation_path = canonical_activation_path(root);
    let activation_dir = activation_path.parent().expect("activation parent");
    std::fs::create_dir_all(activation_dir).expect("create activation dir");
    let activation = json!({
        "schemaId": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_VERSION,
        "schemaAuthority": "https://tao3k.github.io/agent-semantic-protocols/schemas/",
        "protocolId": agent_semantic_hook::HOOK_PROTOCOL_ID,
        "protocolVersion": agent_semantic_hook::HOOK_PROTOCOL_VERSION,
        "projectRoot": root.canonicalize().expect("canonical root").display().to_string(),
        "generatedBy": { "runtime": "asp", "version": "test" },
        "rankers": [],
        "providers": [{
            "manifestId": manifest.manifest_id(),
            "manifestDigest": manifest_digest,
            "languageId": manifest.language_id(),
            "providerId": manifest.provider_id(),
            "binary": manifest.binary(),
            "execution": manifest.execution(),
            "providerCommandPrefix": [],
            "executionCommandDigest": execution_command_digest,
            "searchCapabilities": manifest.search_capabilities(),
            "semanticFactsDescriptor": manifest.semantic_facts_descriptor(),
            "queryPackDescriptor": manifest.query_pack_descriptor(),
            "semanticRegistryDigest": agent_semantic_hook::semantic_registry_digest(),
            "routes": routes,
            "coverage": {
                "packageRoots": ["."],
                "sourceRoots": manifest.source().default_source_roots,
                "configFiles": manifest.source().default_config_files,
                "sourceExtensions": manifest.source().default_extensions,
                "ignoredPathPrefixes": manifest.source().default_ignored_path_prefixes
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
        .find(|manifest| manifest.language_id().as_str() == "rust")
        .expect("rust manifest")
}

fn run_healthcheck(root: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
    let state_home = envs
        .iter()
        .find_map(|(key, value)| (*key == "ASP_STATE_HOME").then(|| PathBuf::from(value)))
        .or_else(|| env::var_os("ASP_STATE_HOME").map(PathBuf::from))
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("HOME").expect("HOME for canonical ASP state"))
                .join(".agent-semantic-protocols")
        });
    let canonical_runtime_asp = state_home.join("runtime/bin/asp");
    let global_bin = root.join("global-bin");
    std::fs::create_dir_all(&global_bin).expect("create isolated global ASP PATH");
    std::os::unix::fs::symlink(&canonical_runtime_asp, global_bin.join("asp"))
        .expect("link isolated global ASP PATH to canonical runtime");
    let path = env::join_paths(std::iter::once(global_bin).chain(env::split_paths(
        &env::var_os("PATH").expect("PATH for healthcheck fixture"),
    )))
    .expect("compose isolated global ASP PATH");

    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command.current_dir(root).arg("healthcheck").args(args);
    command.env("CODEX_HOME", root.join("codex-home"));
    command.env("PATH", path);
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command.output().expect("run asp healthcheck");
    if !output.status.success() {
        eprintln!(
            "healthcheck fixture stdout:\n{}\nhealthcheck fixture stderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
    output
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
