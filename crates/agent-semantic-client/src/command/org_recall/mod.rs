// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Retired `asp org recall` command boundary.
//!
//! The previous implementation scanned the legacy `orgArtifacts/flow/plans`
//! layout and therefore cannot authoritatively observe the current Artifacts
//! model.  Keep the command boundary only to return an explicit typed terminal
//! until a new Runtime-owned recall contract is designed.

mod cli;

pub(crate) use cli::run_org_recall_command;
