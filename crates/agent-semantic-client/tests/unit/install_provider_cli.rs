// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use sha2::Digest;
use sha2::Sha256;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[path = "install_provider_cli/cli_contract.rs"]
mod cli_contract;
#[path = "install_provider_cli/gerbil_release.rs"]
mod gerbil_release;
#[path = "install_provider_cli/receipt_contract.rs"]
mod receipt_contract;

#[test]
#[cfg(unix)]
fn install_language_pinned_release_writes_runtime_bin_package_and_lock() {
    assert_install_pinned_release_writes_runtime_bin_package_and_lock();
}

#[test]
fn document_surfaces_are_not_installable_provider_artifacts() {
    let register = agent_semantic_provider_protocol::parse_provider_install_register(
        include_bytes!("../../../../schemas/provider-install-register.json"),
    )
    .expect("provider install register");
    for language_id in ["org", "md"] {
        assert!(
            register
                .providers
                .iter()
                .all(|registration| registration.language_id != language_id),
            "embedded document surface must not own an installable provider: {language_id}"
        );
    }
}

#[test]
#[cfg(unix)]
fn install_language_pinned_release_ignores_asp_toml_provider_bin() {
    assert_install_language_pinned_release_ignores_asp_toml_provider_bin();
}

#[test]
#[cfg(unix)]
fn install_binary_reconciles_provider_artifacts_without_starting_runtime() {
    let _install_guard = crate::install_binary_test_guard::acquire();
    let root = temp_project_root();
    let state_home = root.join("state");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_asp"))
            .args(["install", "binary"])
            .env("ASP_STATE_HOME", &state_home)
            .env("HOME", root.join("home"))
            .current_dir(&root)
            .output()
            .expect("run ASP binary installation")
    };
    let first = run();
    assert!(
        first.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    let warm = run();
    assert!(
        warm.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&warm.stdout),
        String::from_utf8_lossy(&warm.stderr)
    );
    let stdout = String::from_utf8_lossy(&warm.stdout);
    assert!(
        stdout.contains("providerReconciliation=configured-artifact-authority"),
        "{stdout}"
    );
    assert!(stdout.contains("providerReconciledCount=0"), "{stdout}");
    assert!(
        stdout.contains("activeRuntimeBundleDigest=blake3-256:"),
        "{stdout}"
    );
    assert!(
        !state_home
            .join("runtime/installed-provider-artifacts.json")
            .exists(),
        "binary installation must not publish a second provider serving authority"
    );
    assert!(
        !state_home.join("runtime/server/candidates").exists(),
        "binary installation must not publish a Runtime Server candidate"
    );
    assert!(
        !state_home.join("runtime/server/endpoint.v1.json").exists(),
        "binary installation must not start or publish a Runtime Server"
    );

    std::fs::remove_dir_all(root).expect("remove install binary fixture");
}

fn assert_install_pinned_release_writes_runtime_bin_package_and_lock() {
    let root = temp_project_root();
    let home = root.join("home");
    let release_dir = create_pinned_release_fixture(&root);
    let workspace_decoy = root.join("languages/asp-rust/target/release/asp-rust");
    std::fs::create_dir_all(workspace_decoy.parent().expect("workspace decoy parent"))
        .expect("create workspace decoy parent");
    std::fs::write(&workspace_decoy, b"workspace-decoy\n").expect("write workspace decoy");
    let fake_bin = create_fake_curl_bin(&root);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "rust",
            "--target",
            "x86_64-unknown-linux-gnu",
        ])
        .current_dir(&root)
        .env("HOME", &home)
        .env("PATH", prepend_path(&fake_bin))
        .env("ASP_TEST_RELEASE_DIR", &release_dir)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run asp install language");

    assert!(!output.status.success(), "unpinned fixture was installed");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("checksum mismatch"), "{stderr}");
    assert!(
        !home
            .join(".agent-semantic-protocols/runtime/bin/asp-rust")
            .exists()
    );
    assert_eq!(
        std::fs::read(&workspace_decoy).expect("read workspace decoy"),
        b"workspace-decoy\n"
    );
}

fn assert_install_language_pinned_release_ignores_asp_toml_provider_bin() {
    let root = temp_project_root();
    let home = root.join("home");
    let release_dir = create_pinned_release_fixture(&root);
    let fake_bin = create_fake_curl_bin(&root);
    std::fs::create_dir_all(root.join(".agents")).expect("create .agents");
    std::fs::write(
        root.join(".agents/asp.toml"),
        "[languages.rust]\nbin = \"tools/asp-rust-config\"\n",
    )
    .expect("write asp.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "rust",
            "--target",
            "x86_64-unknown-linux-gnu",
        ])
        .current_dir(&root)
        .env("HOME", &home)
        .env("PATH", prepend_path(&fake_bin))
        .env("ASP_TEST_RELEASE_DIR", &release_dir)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run asp install language");

    assert!(!output.status.success(), "unpinned fixture was installed");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("checksum mismatch"), "{stderr}");
    assert!(
        !root.join("tools/asp-rust-config").exists(),
        "asp.toml language bin must not be an install target"
    );
    assert!(
        !home
            .join(".agent-semantic-protocols/runtime/bin/asp-rust")
            .exists()
    );
}

fn create_pinned_release_fixture(root: &Path) -> PathBuf {
    let release_dir = root.join("release");
    let payload_dir = release_dir.join("payload");
    let binary = payload_dir.join("rs-harness");
    std::fs::create_dir_all(&payload_dir).expect("create release payload dir");
    std::fs::write(
        &binary,
        "#!/bin/sh\nprintf 'provider-ok:%s\\n' \"${1:-missing}\"\n",
    )
    .expect("write fake provider binary");
    make_executable(&binary);

    let archive = release_dir.join("rs-harness-x86_64-unknown-linux-gnu.tar.gz");
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&payload_dir)
        .arg("rs-harness")
        .status()
        .expect("create provider archive");
    assert!(status.success(), "tar failed with status {status}");
    let sha256 = sha256_file(&archive);
    std::fs::write(
        release_dir.join("rs-harness-x86_64-unknown-linux-gnu.tar.gz.sha256"),
        format!("{sha256}  rs-harness-x86_64-unknown-linux-gnu.tar.gz\n"),
    )
    .expect("write provider checksum");
    release_dir
}

fn create_gerbil_pinned_release_fixture(root: &Path) -> PathBuf {
    create_gerbil_release_fixture(root, b"\x7FELFfake-gerbil-native-provider\n")
}

fn create_gerbil_script_release_fixture(root: &Path) -> PathBuf {
    create_gerbil_release_fixture(
        root,
        b"#!/bin/sh\nprintf 'gerbil-provider-ok:%s\\n' \"${1:-missing}\"\n",
    )
}

fn create_gerbil_release_fixture(root: &Path, payload: &[u8]) -> PathBuf {
    let release_dir = root.join("release");
    let payload_dir = release_dir.join("payload");
    let bin_dir = payload_dir.join("bin");
    let binary = bin_dir.join("gerbil-scheme-harness");
    std::fs::create_dir_all(&bin_dir).expect("create Gerbil release bin dir");
    std::fs::write(&binary, payload).expect("write fake Gerbil provider binary");
    make_executable(&binary);

    let archive = release_dir.join("gerbil-scheme-harness-x86_64-unknown-linux-gnu.tar.gz");
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&payload_dir)
        .arg("bin")
        .status()
        .expect("create Gerbil provider archive");
    assert!(status.success(), "tar failed with status {status}");
    let sha256 = sha256_file(&archive);
    std::fs::write(
        release_dir.join("gerbil-scheme-harness-x86_64-unknown-linux-gnu.tar.gz.sha256"),
        format!("{sha256}  gerbil-scheme-harness-x86_64-unknown-linux-gnu.tar.gz\n"),
    )
    .expect("write Gerbil provider checksum");
    release_dir
}

fn create_fake_curl_bin(root: &Path) -> PathBuf {
    let fake_bin = root.join("fake-bin");
    let fake_curl = fake_bin.join("curl");
    std::fs::create_dir_all(&fake_bin).expect("create fake bin dir");
    std::fs::write(
        &fake_curl,
        r#"#!/bin/sh
if [ "$1" != "-fsSL" ] || [ "$2" != "-o" ]; then
  echo "unexpected curl args: $*" >&2
  exit 1
fi
out="$3"
url="$4"
case "$url" in
  https://github.com/tao3k/rust-lang-project-harness/releases/download/v0.1.2/*)
    ;;
  https://github.com/tao3k/asp-gerbil-scheme/releases/download/v0.1.0/*)
    ;;
  *)
    echo "unexpected release url: $url" >&2
    exit 1
    ;;
esac
name="${url##*/}"
case "$name" in
  rs-harness-x86_64-unknown-linux-gnu.tar.gz|rs-harness-x86_64-unknown-linux-gnu.tar.gz.sha256)
    cp "$ASP_TEST_RELEASE_DIR/$name" "$out"
    ;;
  gerbil-scheme-harness-x86_64-unknown-linux-gnu.tar.gz|gerbil-scheme-harness-x86_64-unknown-linux-gnu.tar.gz.sha256)
    cp "$ASP_TEST_RELEASE_DIR/$name" "$out"
    ;;
  *)
    echo "unexpected release asset: $url" >&2
    exit 1
    ;;
esac
"#,
    )
    .expect("write fake curl");
    make_executable(&fake_curl);
    fake_bin
}

fn prepend_path(path: &Path) -> std::ffi::OsString {
    let mut paths = vec![path.to_path_buf()];
    if let Some(existing_path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing_path));
    }
    std::env::join_paths(paths).expect("join PATH")
}

fn sha256_file(path: &Path) -> String {
    let mut file = std::fs::File::open(path).expect("open file for sha256");
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 32 * 1024];
    loop {
        let read = file.read(&mut buffer).expect("read file for sha256");
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    format!("{:x}", hasher.finalize())
}

fn temp_project_root() -> PathBuf {
    let unique = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
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
