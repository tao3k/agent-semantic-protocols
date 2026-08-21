//! Shared black-box support for Protocol's Hook adapter acceptance tests.
//!
//! Policy witness generation remains owned by `agent_semantic_hook`; this module
//! owns only durable fixture setup, process invocation, and protocol receipts.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

pub(crate) fn run_hook(root: &Path, state_home: &Path, activation: &Path, payload: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(root)
        .args([
            "hook",
            "pre-tool",
            "--client",
            "codex",
            "--emit",
            "decision",
            "--activation",
        ])
        .arg(activation)
        .env("ASP_STATE_HOME", state_home)
        .env("PATH", hook_fixture_path(root))
        .env_remove("ASP_NO_AGENT")
        .env_remove("PRJ_CACHE_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn Hook blackbox process");
    child
        .stdin
        .as_mut()
        .expect("Hook stdin")
        .write_all(payload.to_string().as_bytes())
        .expect("write Host envelope");
    let output = child.wait_with_output().expect("wait for Hook decision");
    assert!(
        output.status.success(),
        "Hook blackbox process failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("decode Hook decision")
}

pub(crate) fn run_hook_matrix_parallel(
    root: &Path,
    state_home: &Path,
    activation: &Path,
    payloads: &[Value],
) -> Vec<Value> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let next = AtomicUsize::new(0);
    let results = Mutex::new(vec![None; payloads.len()]);
    let worker_count = 8.min(payloads.len());
    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(payload) = payloads.get(index) else {
                    break;
                };
                let decision = run_hook(root, state_home, activation, payload.clone());
                results.lock().expect("Hook matrix result lock")[index] = Some(decision);
            });
        }
    });
    results
        .into_inner()
        .expect("Hook matrix result lock")
        .into_iter()
        .enumerate()
        .map(|(index, decision)| {
            decision.unwrap_or_else(|| panic!("Hook matrix worker omitted witness {index}"))
        })
        .collect()
}

pub(crate) fn warm_executable_without_hook_state(root: &Path, state_home: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(root)
        .arg("--help")
        .env("ASP_STATE_HOME", state_home)
        .env_remove("ASP_NO_AGENT")
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("warm debug Hook executable pages");
    assert!(
        output.status.success(),
        "Hook executable warmup failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !state_home.join("hooks/cache").exists(),
        "executable warmup must not publish Hook matcher state"
    );
}

pub(crate) fn write_fixture(root: &Path, state_home: &Path) {
    let repository = gix::discover(root).expect("discover durable Hook fixture workspace via Gix");
    assert!(
        repository.worktree().is_some(),
        "Hook fixture must be mounted in a durable workspace worktree"
    );
    for (path, content) in [
        (
            "Cargo.toml",
            "[package]\nname = \"asp-hook-blackbox-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        ),
        ("src/lib.rs", "pub fn blackbox() {}\n"),
        ("src/main.py", "def blackbox():\n    pass\n"),
        ("README.md", "# Hook blackbox\n"),
        ("DESIGN.org", "* Hook blackbox\n"),
        ("Cargo.lock", "# unregistered lockfile witness\n"),
    ] {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().expect("fixture parent"))
            .expect("create fixture parent");
        std::fs::write(path, content).expect("write Hook blackbox owner");
    }
    for language in [
        "rust",
        "typescript",
        "python",
        "julia",
        "gerbil-scheme",
        "md",
        "org",
    ] {
        crate::state_home_fixture::install_provider_script(
            state_home,
            language,
            "#!/bin/sh\nexit 0\n",
        );
    }
    let projector_bin = root.join(".hook-projectors");
    std::fs::create_dir_all(&projector_bin).expect("create projector capability fixture");
    let config = agent_semantic_config::default_hook_client_config_file()
        .expect("load projector capability contracts");
    for binary in config.rules.into_iter().filter_map(|rule| {
        rule.match_config
            .structured_projection
            .map(|projection| projection.binary)
    }) {
        let path = projector_bin.join(binary);
        std::fs::write(&path, "#!/bin/sh\nexit 0\n").expect("write projector capability fixture");
        let mut permissions = std::fs::metadata(&path)
            .expect("read projector capability metadata")
            .permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(path, permissions).expect("make projector capability executable");
    }
    crate::state_home_fixture::write_activation(
        root,
        state_home,
        &[
            "rust",
            "typescript",
            "python",
            "julia",
            "gerbil-scheme",
            "md",
            "org",
        ],
    );
    std::fs::create_dir_all(root.join(".agent-semantic-protocols/hooks"))
        .expect("create Hook config owner");
    std::fs::write(
        root.join(".agent-semantic-protocols/hooks/config.toml"),
        agent_semantic_hook::default_client_config_template(),
    )
    .expect("write Hook config");
}

fn hook_fixture_path(root: &Path) -> std::ffi::OsString {
    let mut paths = vec![root.join(".hook-projectors")];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    std::env::join_paths(paths).expect("join Hook fixture PATH")
}

pub(crate) fn assert_server_absent(state_home: &Path, stage: &str) {
    let server = state_home.join("runtime/server");
    for artifact in ["endpoint.v1.json", "run-intent.v1", "owner-spawn.v1.json"] {
        assert!(
            !server.join(artifact).exists(),
            "Hook created Runtime Server artifact {artifact} at {stage}"
        );
    }
}

pub(crate) fn fixture_root() -> PathBuf {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("protocol crate belongs to the Cargo workspace");
    let fixture_parent = workspace_root.join("target/hook-blackbox-fixtures");
    std::fs::create_dir_all(&fixture_parent).expect("create Hook blackbox fixture parent");
    tempfile::Builder::new()
        .prefix("asp-hook-blackbox-")
        .tempdir_in(fixture_parent)
        .expect("Hook blackbox tempdir")
        .keep()
}
