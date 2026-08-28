//! Compiler-only materialization of the behavior-probe fixture.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static MATERIALIZATION_NONCE: AtomicU64 = AtomicU64::new(0);

fn materialize_probe_root() -> Result<PathBuf, String> {
    let root =
        std::env::temp_dir().join(format!("asp-reader-probe-{}", unsafe { libc::geteuid() }));
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| error.to_string())?;
    Ok(root)
}

pub(super) fn materialize() -> Result<PathBuf, String> {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let root = materialize_probe_root()?;
    let bytes = super::reader_probe_fixture_bytes();
    let expected_digest = blake3::hash(bytes);
    let digest = expected_digest.to_hex();
    let path = root.join(format!("fixture-{digest}"));
    if !path.try_exists().map_err(|error| error.to_string())? {
        let nonce = MATERIALIZATION_NONCE.fetch_add(1, Ordering::Relaxed);
        let candidate = root.join(format!(
            ".fixture-{}-{nonce}-{digest}",
            std::process::id()
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
            .map_err(|error| error.to_string())?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o500))
            .map_err(|error| error.to_string())?;
        match std::fs::hard_link(&candidate, &path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                let _ = std::fs::remove_file(&candidate);
                return Err(error.to_string());
            }
        }
        std::fs::remove_file(&candidate).map_err(|error| error.to_string())?;
    }
    let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
    let actual = std::fs::read(&path).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_file()
        || metadata.permissions().mode() & 0o100 == 0
        || blake3::hash(&actual) != expected_digest
    {
        return Err(format!(
            "Reader probe fixture authority mismatch at {}",
            path.display()
        ));
    }
    Ok(path)
}
