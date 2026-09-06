// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Explicit whole-workspace ASP Rust policy gate.

#[test]
fn asp_workspace_source_policy_is_clean() {
    let _receipt = asp_rust_project_harness_policy::assert_asp_workspace_policy_from_env();
}
