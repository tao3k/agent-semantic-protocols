// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_config::codex_config_plugin_enabled;

#[test]
fn codex_plugin_enabled_is_owned_by_config_parser() {
    let root = tempfile::tempdir().expect("tempdir");
    let config = root.path().join("config.toml");
    std::fs::write(
        &config,
        "[plugins.\"asp-codex-plugin@asp-project\"]\nenabled = true\n",
    )
    .expect("write config");

    assert!(
        codex_config_plugin_enabled(&config, "asp-codex-plugin@asp-project".into())
            .expect("parse config")
    );
    assert!(
        !codex_config_plugin_enabled(&config, "different@plugin".into()).expect("parse config")
    );
    assert!(
        !codex_config_plugin_enabled(&root.path().join("missing.toml"), "anything".into())
            .expect("missing config is disabled")
    );
}
