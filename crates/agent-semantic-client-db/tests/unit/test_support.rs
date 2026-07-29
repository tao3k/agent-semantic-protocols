use std::path::Path;

use tempfile::TempDir;

#[path = "test_support_common.rs"]
mod common;

pub(super) use common::{StateHomeGuard, environment_lock, workspace};

pub(super) struct TestDir(TempDir);

impl TestDir {
    pub(super) fn new(_label: &str) -> Self {
        Self(TempDir::new().expect("create workspace database tempfile"))
    }

    pub(super) fn path(&self) -> &Path {
        self.0.path()
    }
}
