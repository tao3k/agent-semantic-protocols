use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[test]
#[cfg(unix)]
fn install_language_archive_writes_runtime_bin_package_and_lock() {
    assert_install_archive_writes_runtime_bin_package_and_lock();
}

#[test]
#[cfg(unix)]
fn install_language_archive_prefers_asp_toml_provider_bin() {
    assert_install_language_archive_prefers_asp_toml_provider_bin();
}

fn assert_install_archive_writes_runtime_bin_package_and_lock() {
    let root = temp_project_root();
    let home = root.join("home");
    let archive = root.join("release/rs-harness");
    std::fs::create_dir_all(archive.parent().expect("archive parent")).expect("create release dir");
    std::fs::write(
        &archive,
        "#!/bin/sh\nprintf 'provider-ok:%s\\n' \"${1:-missing}\"\n",
    )
    .expect("write fake archive binary");
    make_executable(&archive);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "rust",
            "--rev",
            "refs/tags/vtest",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--archive",
        ])
        .arg(&archive)
        .arg("--project")
        .arg(&root)
        .env("HOME", &home)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run asp install language");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[asp-install]"), "{stdout}");
    assert!(stdout.contains("rev=refs/tags/vtest"), "{stdout}");

    let runtime = root.join(".cache/agent-semantic-protocol/runtime");
    let bin = home.join(".local/bin/rs-harness");
    let package_binary =
        runtime.join("providers/rust/refs_tags_vtest/x86_64-unknown-linux-gnu/rs-harness");
    let lock = runtime.join("providers/rust.lock.toml");
    assert!(bin.is_file(), "missing runtime bin {}", bin.display());
    assert!(
        package_binary.is_file(),
        "missing package binary {}",
        package_binary.display()
    );
    let lock_contents = std::fs::read_to_string(&lock).expect("read install lock");
    assert!(lock_contents.contains("rev = \"refs/tags/vtest\""));
    assert!(lock_contents.contains("packagePath = "));

    let provider_output = Command::new(&bin)
        .arg("probe")
        .output()
        .expect("run installed provider");
    assert!(
        provider_output.status.success(),
        "provider stderr: {}",
        String::from_utf8_lossy(&provider_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&provider_output.stdout),
        "provider-ok:probe\n"
    );
}

fn assert_install_language_archive_prefers_asp_toml_provider_bin() {
    let root = temp_project_root();
    let home = root.join("home");
    let archive = root.join("release/rs-harness");
    std::fs::create_dir_all(archive.parent().expect("archive parent")).expect("create release dir");
    std::fs::write(
        &archive,
        "#!/bin/sh\nprintf 'provider-ok:%s\\n' \"${1:-missing}\"\n",
    )
    .expect("write fake archive binary");
    make_executable(&archive);
    std::fs::write(
        root.join("asp.toml"),
        "[languages.rust]\nbin = \"tools/rs-harness-config\"\n",
    )
    .expect("write asp.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "rust",
            "--rev",
            "refs/tags/vconfig",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--archive",
        ])
        .arg(&archive)
        .arg("--project")
        .arg(&root)
        .env("HOME", &home)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run asp install language");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("installTargetSource=asp.toml"), "{stdout}");

    let bin = root.join("tools/rs-harness-config");
    assert!(bin.is_file(), "missing configured bin {}", bin.display());

    let provider_output = Command::new(&bin)
        .arg("probe")
        .output()
        .expect("run configured provider");
    assert!(
        provider_output.status.success(),
        "provider stderr: {}",
        String::from_utf8_lossy(&provider_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&provider_output.stdout),
        "provider-ok:probe\n"
    );
}

fn temp_project_root() -> PathBuf {
    let unique = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "asp-install-provider-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos(),
        unique,
    ));
    std::fs::create_dir_all(&root).expect("create temp root");
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(&root)
        .status()
        .expect("git init");
    root
}

fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .expect("provider metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("provider permissions");
}
