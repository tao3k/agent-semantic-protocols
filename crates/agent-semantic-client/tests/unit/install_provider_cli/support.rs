use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

pub(super) fn create_pinned_release_fixture(root: &Path) -> PathBuf {
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

pub(super) fn create_gerbil_pinned_release_fixture(root: &Path) -> PathBuf {
    create_gerbil_release_fixture(root, b"\x7FELFfake-gerbil-native-provider\n")
}

pub(super) fn create_gerbil_script_release_fixture(root: &Path) -> PathBuf {
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

pub(super) fn create_fake_curl_bin(root: &Path) -> PathBuf {
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
  https://github.com/tao3k/gerbil-scheme-language-project-harness/releases/download/v0.1.0/*)
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

pub(super) fn prepend_path(path: &Path) -> std::ffi::OsString {
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

pub(super) fn sorted_file_names(path: &Path) -> Vec<String> {
    let mut entries = std::fs::read_dir(path)
        .expect("read dir")
        .map(|entry| {
            entry
                .expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

pub(super) fn temp_project_root() -> PathBuf {
    let unique = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let temp_dir = std::env::temp_dir();
    let canonical_temp_dir = std::fs::canonicalize(&temp_dir).unwrap_or(temp_dir);
    let root = canonical_temp_dir.join(format!(
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

pub(super) fn create_fake_tool_bin(root: &Path, name: &str, body: &str) -> PathBuf {
    let bin_dir = root.join(".fake-bin");
    std::fs::create_dir_all(&bin_dir).expect("create fake bin dir");
    let tool = bin_dir.join(name);
    std::fs::write(&tool, body).expect("write fake tool");
    make_executable(&tool);
    tool
}

pub(super) fn write_workspace_source_anchors(root: &Path, anchors: &[&str]) {
    for anchor in anchors {
        let path = root.join(anchor);
        std::fs::create_dir_all(path.parent().expect("source anchor parent"))
            .expect("create source anchor parent");
        std::fs::write(&path, format!("# fixture anchor: {anchor}\n"))
            .expect("write source snapshot anchor");
    }
}

pub(super) fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .expect("provider metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("provider permissions");
}
