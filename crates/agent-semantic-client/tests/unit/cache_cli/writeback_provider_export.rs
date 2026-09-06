// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::provider_command_prefix;

#[test]
fn provider_command_requires_runtime_or_configured_control_plane() {
    let _lock = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let home = TestRoot::new();
    let mut provider = crate::test_support::resolved_provider("rust");
    provider.runtime_command_argv = None;
    provider.provider_command_prefix.clear();
    let home_binary = home
        .path()
        .join(".local")
        .join("bin")
        .join(&provider.binary);
    std::fs::create_dir_all(home_binary.parent().expect("home bin parent")).expect("home bin");
    std::fs::write(&home_binary, b"legacy home binary").expect("home binary");
    let _home = crate::test_support::EnvVarGuard::set("HOME", home.path());

    assert_eq!(provider_command_prefix(&provider), None);

    provider.provider_command_prefix = vec!["configured-provider".to_string()];
    assert_eq!(
        provider_command_prefix(&provider),
        Some(vec!["configured-provider".to_string()])
    );

    provider.runtime_command_argv = Some(vec!["state-home-provider".to_string()]);
    assert_eq!(
        provider_command_prefix(&provider),
        Some(vec!["state-home-provider".to_string()])
    );
}

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "asp-provider-authority-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("fake home");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
