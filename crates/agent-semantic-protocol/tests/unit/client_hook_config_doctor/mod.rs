use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const PROBE_SENTINEL: &str = "ASP_CODEX_HOOK_ENFORCEMENT_PROBE_SENTINEL_DO_NOT_LEAK";

mod basic;
mod runtime;
mod trust;

fn write_client_config(root: &std::path::Path, content: &str) {
    write_agent_registry(root);
    let config_path = asp_state_home(root).join("hooks/config.toml");
    std::fs::create_dir_all(config_path.parent().expect("config parent"))
        .expect("create config dir");
    std::fs::write(config_path, content).expect("write client config");
}

fn write_agent_registry(root: &std::path::Path) {
    let agents = root.join("agents");
    std::fs::create_dir_all(&agents).expect("create project agent registry");
    for (name, bytes) in [
        (
            "config.toml",
            include_bytes!("../../../../../agents/config.toml").as_slice(),
        ),
        (
            "asp_explorer_codex.toml",
            include_bytes!("../../../../../agents/asp_explorer_codex.toml").as_slice(),
        ),
        (
            "asp_explorer_claude.md",
            include_bytes!("../../../../../agents/asp_explorer_claude.md").as_slice(),
        ),
        (
            "asp_testing_codex.toml",
            include_bytes!("../../../../../agents/asp_testing_codex.toml").as_slice(),
        ),
        (
            "asp_testing_claude.md",
            include_bytes!("../../../../../agents/asp_testing_claude.md").as_slice(),
        ),
    ] {
        std::fs::write(agents.join(name), bytes).expect("write project agent registry entry");
    }
}

fn asp_state_home(root: &std::path::Path) -> std::path::PathBuf {
    root.join(".agent-semantic-protocols")
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
}

fn write_codex_plugin_fixture(root: &std::path::Path) {
    let plugin_root = root.join("asp-codex-plugin");
    for (relative, bytes) in [
        (
            ".codex-plugin/plugin.json",
            include_bytes!("../../../../../asp-codex-plugin/.codex-plugin/plugin.json").as_slice(),
        ),
        (
            "hooks/hooks.json",
            include_bytes!("../../../../../asp-codex-plugin/hooks/hooks.json").as_slice(),
        ),
    ] {
        let path = plugin_root.join(relative);
        std::fs::create_dir_all(path.parent().expect("plugin fixture parent"))
            .expect("create plugin fixture dir");
        std::fs::write(path, bytes).expect("write plugin fixture artifact");
    }
    write_codex_project_plugin_config(root);
}

fn write_codex_global_inline_config(root: &std::path::Path, asp_binary: &std::path::Path) {
    let codex_home = root.join(".codex-home");
    std::fs::create_dir_all(&codex_home).expect("create isolated Codex home");
    let config_path = codex_home.join("config.toml");
    let block = agent_semantic_hook::codex_global_hook_block_with_binary(Some(asp_binary));
    let config = agent_semantic_hook::merge_codex_global_hook_trust_config(
        &format!("[features]\nhooks = true\nunified_exec = true\n\n{block}\n"),
        &config_path,
        Some(asp_binary),
    );
    std::fs::write(config_path, config).expect("write global inline Hook config");
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
    let state_home = asp_state_home(root);
    crate::state_home_fixture::install_provider_script(&state_home, "rust", "#!/bin/sh\nexit 0\n");
    crate::state_home_fixture::write_activation(root, &state_home, &["rust"])
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
