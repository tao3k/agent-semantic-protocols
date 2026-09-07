// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_config::HookClientConfigFile;

#[test]
fn legacy_global_wrapper_match_is_rejected() {
    let _config = agent_semantic_config::default_hook_client_config_file()
        .expect("parse canonical default hook client config");

    let error = toml::from_str::<HookClientConfigFile>("wrapper_match = \"disable\"").unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains("unknown field `wrapper_match`"),
        "error={message}"
    );
}
