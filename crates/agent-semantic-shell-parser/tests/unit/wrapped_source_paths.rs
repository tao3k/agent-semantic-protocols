// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_shell_parser::command_source_paths;
use agent_semantic_shell_parser::embedded_literal_candidates;

#[test]
fn arbitrary_wrappers_preserve_nested_source_read_operands() {
    let source_path = "crates/agent-semantic-hook/src/lib.rs";
    let commands = [
        format!("custom-runner Path('{source_path}').read_text()"),
        format!("custom-runner read_to_string('{source_path}')"),
    ];

    for command in commands {
        let embedded = embedded_literal_candidates(std::slice::from_ref(&command));
        assert!(
            embedded.iter().any(|path| path == source_path),
            "nested source read omitted its quoted operand: command={command} candidates={embedded:?}"
        );
        let wrappers = [
            format!("direnv exec . {command}"),
            format!("organization-local-wrapper --profile ci {command}"),
            format!("future-wrapper alpha beta {command}"),
            format!("env ASP_MATCH_SCENARIO=1 {command}"),
            format!("bash -lc \"{command}\""),
        ];
        for wrapped in wrappers {
            let paths = command_source_paths(&wrapped, &[]);
            assert!(
                paths.iter().any(|path| path == source_path),
                "wrapped source read omitted its operand: command={wrapped} paths={paths:?}"
            );
        }
    }
}
