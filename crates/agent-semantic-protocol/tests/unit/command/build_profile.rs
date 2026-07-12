use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace root")
}

#[test]
fn version_reports_the_compiled_artifact_profile() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["--version", "--profile"])
        .output()
        .expect("run asp --version --profile");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let expected = if cfg!(debug_assertions) {
        "debug\n"
    } else {
        "release\n"
    };
    assert_eq!(stdout, expected);
}

#[test]
fn require_release_fails_closed_for_debug_artifacts() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["--version", "--require-release"])
        .output()
        .expect("run asp --version --require-release");

    if cfg!(debug_assertions) {
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
        assert!(
            stderr.contains("[asp-build-profile-error] expected=release actual=debug"),
            "stderr={stderr}"
        );
        assert!(
            stderr.contains("nextCommand=just agent-tools-install-protocol"),
            "stderr={stderr}"
        );
    } else {
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
        assert!(stdout.contains("profile=release"), "stdout={stdout}");
    }
}

#[test]
fn global_install_checks_release_profile_before_and_after_copy() {
    let justfile = fs::read_to_string(workspace_root().join("justfile")).expect("read justfile");
    let recipe = justfile
        .split("agent-tools-install-protocol bin_dir=\"\":")
        .nth(1)
        .and_then(|tail| tail.split("agent-tools-install-protocol-debug").next())
        .expect("protocol install recipe");

    assert_eq!(
        recipe.matches("--version --require-release").count(),
        2,
        "release profile must be checked before and after installation"
    );
    assert!(recipe.contains("target/release/asp"));
    assert!(!recipe.contains("target/debug/asp"));
}
