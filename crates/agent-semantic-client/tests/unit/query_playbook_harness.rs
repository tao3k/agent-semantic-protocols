// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Focused Query Playbook process-boundary tests.
//!
//! This small harness avoids relinking the repository-wide `unit_test` binary
//! when only the Query facade and Runtime handoff contract changed.

#[path = "projection_presentation.rs"]
mod projection_presentation;
#[path = "query_playbook_cli.rs"]
mod query_playbook_cli;
#[path = "query_selector_handoff.rs"]
mod query_selector_handoff;
