//! Opt-in Codex CLI hook enforcement smoke.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

const ENABLE_ENV: &str = "ASP_CODEX_CLI_E2E";
const CODEX_CLI_ENV: &str = "ASP_CODEX_CLI";
const DOCTOR_PROBE_ENV: &str = "ASP_CODEX_CLI_ENFORCEMENT_PROBE";

#[test]
fn codex_cli_hook_enforcement_probe_reports_real_status_when_enabled() {
    if env::var(ENABLE_ENV).ok().as_deref() != Some("1") {
        eprintln!("skipping Codex CLI E2E; set {ENABLE_ENV}=1 to run");
        return;
    }

    let Some(codex_cli) = codex_cli_path() else {
        panic!("Codex CLI not found; set {CODEX_CLI_ENV}=/path/to/codex");
    };

    let root = temp_project_root("codex-cli-hook-e2e");
    let bin_dir = root.join(".asp-test-bin");
    write_project_fixture(&root);
    write_asp_wrapper(&bin_dir, Path::new(env!("CARGO_BIN_EXE_asp")));
    let codex_home = write_codex_home_fixture(&root);

    let test_path = prepend_path(&bin_dir);
    let install = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &test_path)
        .env("CODEX_HOME", &codex_home)
        .args(["install", "plugin", "--codex", "."])
        .output()
        .expect("run asp install plugin");
    assert!(
        install.status.success(),
        "asp install plugin failed: stdout={} stderr={}",
        String::from_utf8_lossy(&install.stdout),
        String::from_utf8_lossy(&install.stderr)
    );

    let output = run_hook_doctor_probe(&root, &test_path, &codex_home, &codex_cli);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "asp hook doctor probe failed: stdout={stdout} stderr={stderr}"
    );
    if stdout.contains("enforcement=enforced") {
        assert!(
            stdout.contains("enforcementReason=hook-deny-observed"),
            "enforced Codex CLI probe did not report the deny reason: stdout={stdout}"
        );
        assert!(
            stdout.contains("sentinel=false"),
            "enforced Codex CLI probe leaked the protected source sentinel: stdout={stdout}"
        );
    } else {
        assert!(
            stdout.contains("enforcement=configured-but-not-enforced"),
            "Codex CLI probe must either enforce or report a fail-safe non-enforced state: stdout={stdout}"
        );
        assert!(
            stdout.contains("enforcementReason=source-sentinel-leaked")
                || stdout.contains("enforcementReason=hook-deny-not-observed"),
            "non-enforced Codex CLI probe did not explain the hook gap: stdout={stdout}"
        );
    }

    let _ = fs::remove_dir_all(root);
}

fn run_hook_doctor_probe(
    root: &Path,
    test_path: &std::ffi::OsStr,
    codex_home: &Path,
    codex_cli: &Path,
) -> Output {
    let activation = root.join(".cache/agent-semantic-protocol/hooks/activation.json");
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command
        .current_dir(root)
        .env("PATH", test_path)
        .env("CODEX_HOME", codex_home)
        .env(DOCTOR_PROBE_ENV, "1")
        .env(CODEX_CLI_ENV, codex_cli)
        .args([
            "hook",
            "doctor",
            "--client",
            "codex",
            "--activation",
            activation.to_str().expect("activation path is utf8"),
            ".",
        ]);
    command.output().expect("run asp hook doctor")
}

fn codex_cli_path() -> Option<PathBuf> {
    env::var_os(CODEX_CLI_ENV)
        .map(PathBuf::from)
        .or_else(|| command_path("codex"))
}

fn command_path(command: &str) -> Option<PathBuf> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {command}"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    let path = path.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

fn write_codex_home_fixture(root: &Path) -> PathBuf {
    let codex_home = root.join(".codex-home");
    fs::create_dir_all(&codex_home).expect("create isolated CODEX_HOME");
    link_or_copy_auth_json(&codex_home);
    fs::write(
        codex_home.join("config.toml"),
        codex_home_config_for_project(root),
    )
    .expect("write isolated CODEX_HOME config");
    codex_home
}

fn link_or_copy_auth_json(codex_home: &Path) {
    let source = real_codex_home().join("auth.json");
    assert!(
        source.is_file(),
        "Codex auth not found at {}; run `codex login` or set CODEX_HOME",
        source.display()
    );
    let target = codex_home.join("auth.json");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&source, &target).expect("symlink auth.json");
    #[cfg(not(unix))]
    fs::copy(&source, &target).expect("copy auth.json");
}

fn real_codex_home() -> PathBuf {
    env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
        .expect("CODEX_HOME or HOME must be set")
}

fn codex_home_config_for_project(root: &Path) -> String {
    let mut project_keys = vec![root.to_path_buf()];
    if let Ok(canonical) = fs::canonicalize(root)
        && canonical != root
    {
        project_keys.push(canonical);
    }
    let mut config = "[features]\nhooks = true\nunified_exec = true\n\n".to_string();
    for project_key in project_keys {
        config.push_str(&format!(
            "[projects.{}]\ntrust_level = \"trusted\"\n\n",
            toml_quoted_key(&project_key.display().to_string())
        ));
    }
    config
}

fn toml_quoted_key(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn write_project_fixture(root: &Path) {
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"codex_cli_hook_e2e\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Cargo.toml");
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("src/lib.rs"),
        "pub const SECRET: &str = \"ASP_CODEX_CLI_E2E_SOURCE_SENTINEL\";\n",
    )
    .expect("write src/lib.rs");
    fs::write(
        root.join("src/hook_warmup.rs"),
        "pub const WARMUP: &str = \"ASP_CODEX_CLI_E2E_WARMUP_SOURCE\";\n",
    )
    .expect("write src/hook_warmup.rs");
    let _ = Command::new("git")
        .current_dir(root)
        .args(["init", "-q"])
        .output();
}

fn write_asp_wrapper(bin_dir: &Path, asp_path: &Path) {
    fs::create_dir_all(bin_dir).expect("create asp wrapper dir");
    let wrapper_path = bin_dir.join("asp");
    fs::write(
        &wrapper_path,
        format!("#!/bin/sh\nexec {:?} \"$@\"\n", asp_path),
    )
    .expect("write asp wrapper");
    make_executable(&wrapper_path);
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path).expect("wrapper metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod asp wrapper");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

fn prepend_path(first: &Path) -> std::ffi::OsString {
    let mut paths = vec![first.to_path_buf()];
    if let Some(existing) = env::var_os("PATH") {
        paths.extend(env::split_paths(&existing));
    }
    env::join_paths(paths).expect("join PATH")
}

fn temp_project_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = env::temp_dir().join(format!("asp-{label}-{unique}"));
    fs::create_dir_all(&root).expect("create temp root");
    root
}
