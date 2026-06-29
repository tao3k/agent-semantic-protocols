use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

pub(crate) static CACHE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    pub(crate) fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let previous = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => unsafe {
                std::env::set_var(self.key, value);
            },
            None => unsafe {
                std::env::remove_var(self.key);
            },
        }
    }
}

pub(crate) fn v2_cache_root(workspace_state_root: &Path) -> PathBuf {
    workspace_state_root.join("live").join("client")
}

pub(crate) fn artifacts_root_from_cache_root(cache_root: &Path) -> PathBuf {
    let live_dir = cache_root.parent().expect("cache root live dir");
    assert_eq!(
        cache_root.file_name().and_then(|name| name.to_str()),
        Some("client")
    );
    assert_eq!(
        live_dir.file_name().and_then(|name| name.to_str()),
        Some("live")
    );
    live_dir
        .parent()
        .expect("cache root workspace dir")
        .join("artifacts")
}
