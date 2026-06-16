use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{OutputMode, ProviderProcessLimits, ProviderProcessSpec, StdinMode};

pub(super) fn temp_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("agent-provider-transport-{name}-{unique}"));
    fs::create_dir_all(&path).expect("create temp dir");
    path.canonicalize().unwrap_or(path)
}

pub(super) fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, body).expect("write script");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("chmod");
    path
}

pub(super) fn spec(program: PathBuf, cwd: PathBuf) -> ProviderProcessSpec {
    ProviderProcessSpec {
        program: program.display().to_string(),
        args: Vec::new(),
        cwd,
        env: BTreeMap::new(),
        stdin: StdinMode::Closed,
        stdout: OutputMode::Capture,
        stderr: OutputMode::Capture,
        limits: ProviderProcessLimits::default(),
    }
}
