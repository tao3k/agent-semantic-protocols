// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! State Home access for the single global ASP configuration contract.

use std::io::ErrorKind;
use std::path::Path;

use agent_semantic_config::runtime_dev::AspGlobalConfig;
use agent_semantic_config::runtime_dev::parse_asp_global_config;

pub fn load_asp_global_config(state_home: &Path) -> Result<AspGlobalConfig, String> {
    let path = crate::StateHomeLayout::new(state_home)
        .control()
        .asp_config();
    let input = match std::fs::read_to_string(&path) {
        Ok(input) => input,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(format!(
                "read global ASP configuration {}: {error}",
                path.display()
            ));
        }
    };
    parse_asp_global_config(&input)
}

pub async fn load_asp_global_config_async(state_home: &Path) -> Result<AspGlobalConfig, String> {
    let path = crate::StateHomeLayout::new(state_home)
        .control()
        .asp_config();
    let input = match tokio::fs::read_to_string(&path).await {
        Ok(input) => input,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(format!(
                "read global ASP configuration {}: {error}",
                path.display()
            ));
        }
    };
    parse_asp_global_config(&input)
}
