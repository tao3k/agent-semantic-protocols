// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed CLI arguments for State Home garbage collection.

use std::path::PathBuf;

/// Parsed arguments for canonical State Home retirement.
#[derive(Clone, Debug, Eq, PartialEq, clap::Args)]
pub(crate) struct CacheCleanArgs {
    /// Retain disappeared temporary-workspace cache for DAYS days (defaults to 1).
    #[arg(
        long,
        required = true,
        value_name = "DAYS",
        num_args = 0..=1,
        default_missing_value = "1",
        value_parser = clap::value_parser!(u64).range(1..)
    )]
    pub(crate) day: u64,

    /// Select exactly one catalog workspace digest.
    #[arg(long, value_name = "BLAKE3_DIGEST", conflicts_with_all = ["workspace_root", "object_id"])]
    pub(crate) workspace_digest: Option<String>,

    /// Resolve and select exactly one canonical workspace root.
    #[arg(long, value_name = "PATH", conflicts_with_all = ["workspace_digest", "object_id"])]
    pub(crate) workspace_root: Option<PathBuf>,

    /// Select exactly one retained catalog object.
    #[arg(long, value_name = "OBJECT_ID", conflicts_with_all = ["workspace_digest", "workspace_root"])]
    pub(crate) object_id: Option<String>,
}

/// Applying cleanup requires an explicit retention flag so a bare `cache clean`
/// invocation cannot delete state accidentally.
#[derive(Clone, Debug, Eq, PartialEq, clap::Parser)]
#[command(
    name = "clean",
    bin_name = "asp cache clean",
    about = "Retire catalog-selected State Home objects after the retention window",
    disable_version_flag = true
)]
struct CacheCleanCli {
    #[command(flatten)]
    args: CacheCleanArgs,
}

/// Return the cleanup command definition shared by execution parsing and help.
pub fn cache_clean_clap_command() -> clap::Command {
    <CacheCleanCli as clap::CommandFactory>::command()
}

pub(crate) fn parse_cache_clean_args(args: &[String]) -> Result<Option<CacheCleanArgs>, String> {
    let argv = std::iter::once("asp cache clean").chain(args.iter().map(String::as_str));
    match cache_clean_clap_command().try_get_matches_from(argv) {
        Ok(matches) => {
            let parsed = <CacheCleanCli as clap::FromArgMatches>::from_arg_matches(&matches)
                .map_err(|error| error.to_string())?;
            Ok(Some(parsed.args))
        }
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            error.print().map_err(|print_error| {
                format!("failed to print cache clean help: {print_error}")
            })?;
            Ok(None)
        }
        Err(error) => Err(error.to_string()),
    }
}
