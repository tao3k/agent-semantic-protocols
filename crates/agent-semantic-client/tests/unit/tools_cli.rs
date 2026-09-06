// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::path::Path;

use crate::tools_cli::run_tools;

#[test]
fn tools_cli_rejects_unknown_subcommand() {
    let error = run_tools(Path::new("."), &["status".to_string()])
        .expect_err("unknown tools subcommand should fail");

    assert_eq!(error, "usage: asp tools doctor [PROJECT_ROOT]");
}
