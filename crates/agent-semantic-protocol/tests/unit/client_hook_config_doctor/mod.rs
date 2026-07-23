use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_hook::{
    builtin_provider_manifests, codex_hook_block, merge_codex_config, provider_manifest_digest,
};
use serde_json::json;

const PROBE_SENTINEL: &str = "ASP_CODEX_HOOK_ENFORCEMENT_PROBE_SENTINEL_DO_NOT_LEAK";

mod basic;
mod runtime;
mod trust;

fn write_client_config(root: &std::path::Path, content: &str) {
    let config_path = asp_state_home(root).join("hooks/config.toml");
    std::fs::create_dir_all(config_path.parent().expect("config parent"))
        .expect("create config dir");
    std::fs::write(
        config_path,
        format!(
            r#"{content}
[agents]

[[agents.residentAgents]]
name = "asp-explore"
role = "asp_explorer"
codexAgentName = "asp_explorer"
roles = ["subagent", "search"]
permissions = ["read-only"]

[[agents.residentAgents]]
name = "asp-testing"
role = "asp_testing"
codexAgentName = "asp_testing"
roles = ["subagent", "testing", "build"]
permissions = ["workspace-write"]
"#
        ),
    )
    .expect("write client config");
}

fn asp_state_home(root: &std::path::Path) -> std::path::PathBuf {
    root.join(".agent-semantic-protocols")
}

fn write_codex_project_config(root: &std::path::Path) {
    let config_path = root.join(".codex/config.toml");
    std::fs::create_dir_all(config_path.parent().expect("project config parent"))
        .expect("create project config dir");
    std::fs::write(config_path, merge_codex_config("", &codex_hook_block(root)))
        .expect("write project Codex config");
}

fn write_codex_project_plugin_config(root: &std::path::Path) {
    let config_path = root.join(".codex/config.toml");
    std::fs::create_dir_all(config_path.parent().expect("project config parent"))
        .expect("create project config dir");
    std::fs::write(
        &config_path,
        r#"[plugins."asp-codex-plugin@asp-project"]
enabled = true
"#,
    )
    .expect("write project plugin Codex config");
    let hooks_path = root
        .join(".codex")
        .join("plugins")
        .join("cache")
        .join("asp-project")
        .join("asp-codex-plugin")
        .join("0.1.0")
        .join("hooks")
        .join("hooks.json");
    std::fs::create_dir_all(hooks_path.parent().expect("plugin hooks parent"))
        .expect("create project plugin hooks dir");
    std::fs::write(hooks_path, "{}\n").expect("write project plugin hooks");
}

fn write_stale_codex_home_config(root: &std::path::Path) {
    let codex_home = root.join(".codex-home");
    std::fs::create_dir_all(&codex_home).expect("create isolated Codex home");
    let config_path =
        std::fs::canonicalize(root.join(".codex/config.toml")).expect("canonical config path");
    let project_root = config_path
        .parent()
        .and_then(std::path::Path::parent)
        .expect("canonical project root");
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            "[projects.{}]\ntrust_level = \"trusted\"\n\n[hooks.state.\"{}:pre_tool_use:0:0\"]\ntrusted_hash = \"sha256:old\"\n",
            toml_basic_string(&project_root.display().to_string()),
            config_path.display()
        ),
    )
    .expect("write stale Codex home config");
}

fn toml_basic_string(value: &str) -> String {
    let mut output = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if c.is_control() => output.push_str(&format!("\\u{:04X}", c as u32)),
            c => output.push(c),
        }
    }
    output.push('"');
    output
}

fn write_activation(root: &std::path::Path) -> PathBuf {
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
    activation_path
}

fn run_doctor(root: &std::path::Path, activation_path: &std::path::Path) -> std::process::Output {
    run_doctor_with_env(root, activation_path, &[], &[], None)
}

fn run_doctor_strict(
    root: &std::path::Path,
    activation_path: &std::path::Path,
) -> std::process::Output {
    run_doctor_with_env_and_args(
        root,
        activation_path,
        &[],
        &[],
        None,
        &["--strict-contract"],
    )
}

fn run_doctor_with_env(
    root: &std::path::Path,
    activation_path: &std::path::Path,
    envs: &[(&str, &str)],
    env_paths: &[(&str, &str)],
    path_prefix: Option<&std::path::Path>,
) -> std::process::Output {
    run_doctor_with_env_and_args(root, activation_path, envs, env_paths, path_prefix, &[])
}

fn run_doctor_with_env_and_args(
    root: &std::path::Path,
    activation_path: &std::path::Path,
    envs: &[(&str, &str)],
    env_paths: &[(&str, &str)],
    path_prefix: Option<&std::path::Path>,
    extra_args: &[&str],
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
    command.args(extra_args);
    command.env("CODEX_HOME", root.join(".codex-home"));
    command.env("ASP_STATE_HOME", asp_state_home(root));
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
    let routes =
        agent_semantic_hook::materialize_provider_routes(&manifest).expect("provider routes");
    serde_json::to_string_pretty(&json!({
        "schemaId": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_VERSION,
        "schemaAuthority": "https://tao3k.github.io/agent-semantic-protocols/schemas/",
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
            "execution": manifest.execution,
            "providerCommandPrefix": [],
            "executionCommandDigest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "searchCapabilities": manifest.search_capabilities,
            "semanticFactsDescriptor": manifest.semantic_facts_descriptor,
            "queryPackDescriptor": manifest.query_pack_descriptor,
            "semanticRegistryDigest": agent_semantic_hook::semantic_registry_digest(),
            "routes": routes,
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
