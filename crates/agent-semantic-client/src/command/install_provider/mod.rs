// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Provider installation branch boundary.

use crate::command::protocol_binary;

mod archive;
mod binary;
mod cli_support;
mod core;
mod development;
mod reconciliation;
mod release;
mod target;
mod workspace;
mod workspace_receipt;

pub(crate) use binary::run_install_binary as run_hook_refresh;
pub(crate) use core::run_install_command;
