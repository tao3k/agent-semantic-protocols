// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::PathBuf;

pub(crate) struct CacheTestLock(std::sync::Mutex<()>);

impl CacheTestLock {
    pub(crate) const fn new() -> Self {
        Self(std::sync::Mutex::new(()))
    }

    pub(crate) fn lock(&self) -> Result<std::sync::MutexGuard<'_, ()>, std::convert::Infallible> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner))
    }
}

pub(crate) static CACHE_TEST_LOCK: CacheTestLock = CacheTestLock::new();

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

pub(crate) fn owner_backed_temp_root(label: &str) -> PathBuf {
    static FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static FIXTURE_BASE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

    let fixture_base = FIXTURE_BASE.get_or_init(|| {
        let repository = gix::discover(env!("CARGO_MANIFEST_DIR"))
            .expect("discover the owner-backed client test repository with Gix");
        let worktree = repository
            .worktree()
            .expect("client tests require a non-bare owner checkout");
        let base = worktree.base().join("target/asp-live-project-fixtures");
        std::fs::create_dir_all(&base).expect("create owner-backed client fixture root");
        base
    });
    let fixture_id = FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = fixture_base.join(format!(
        "agent-semantic-client-{label}-{}-{fixture_id}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create isolated owner-backed client fixture");
    gix::init(&root).expect("initialize owner-backed client fixture with Gix");
    root
}
