// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Optional user hook configuration location under the ASP state home.

use std::env;
use std::path::PathBuf;

const ASP_STATE_HOME_ENV: &str = "ASP_STATE_HOME";
const DEFAULT_STATE_HOME_DIR: &str = ".agent-semantic-protocols";

pub fn default_global_client_config_path() -> Option<PathBuf> {
    let state_home = env::var_os(ASP_STATE_HOME_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join(DEFAULT_STATE_HOME_DIR))
        })?;
    Some(
        agent_semantic_artifacts::StateHomeLayout::new(state_home)
            .control()
            .hook_client_config(),
    )
}
