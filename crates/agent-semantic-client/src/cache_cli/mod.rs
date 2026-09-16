// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Cache command helpers for the `asp` client.

pub(crate) mod clean_args;
mod command;
pub use clean_args::cache_clean_clap_command;
mod clean_command;

#[cfg(test)]
#[path = "../../tests/unit/cache_cli/clean_args.rs"]
mod clean_args_tests;

pub(crate) use command::run_cache;
