// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

pub fn semantic_shell_tokens(command: &str) -> Vec<String> {
    agent_semantic_shell_parser::parse_bash_command_candidates(command)
        .unwrap_or_default()
        .into_iter()
        .flat_map(|stage| stage.words().to_vec())
        .collect()
}
