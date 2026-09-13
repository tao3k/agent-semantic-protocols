// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::subagent_model_arg;

#[test]
fn codex_plugin_install_cannot_override_registry_owned_models() {
    assert!(subagent_model_arg("codex", None).is_err());
    assert!(subagent_model_arg("codex", Some("gpt-5.6-luna")).is_err());
}
