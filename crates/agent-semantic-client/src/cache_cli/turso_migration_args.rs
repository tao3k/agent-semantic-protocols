//! Typed `asp cache migrate` command ownership.

/// Supported physical client-DB migration target.
#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::Subcommand)]
pub(crate) enum CacheMigrationTarget {
    /// Replay the logical v1 database into native Turso 0.7 files.
    #[command(name = "turso-0-7")]
    Turso07,
}

/// Clap-owned cache migration command.
#[derive(Clone, Debug, Eq, PartialEq, clap::Parser)]
#[command(
    name = "migrate",
    bin_name = "asp cache migrate",
    about = "Migrate the canonical project client database",
    disable_version_flag = true
)]
struct CacheMigrationCli {
    #[command(subcommand)]
    target: CacheMigrationTarget,
}

/// Return the command definition shared by execution parsing and help.
pub fn cache_migration_clap_command() -> clap::Command {
    <CacheMigrationCli as clap::CommandFactory>::command()
}

pub(crate) fn parse_cache_migration_target(
    args: &[String],
) -> Result<Option<CacheMigrationTarget>, String> {
    let argv = std::iter::once("asp cache migrate").chain(args.iter().map(String::as_str));
    match cache_migration_clap_command().try_get_matches_from(argv) {
        Ok(matches) => {
            let parsed = <CacheMigrationCli as clap::FromArgMatches>::from_arg_matches(&matches)
                .map_err(|error| error.to_string())?;
            Ok(Some(parsed.target))
        }
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            error.print().map_err(|print_error| {
                format!("failed to print cache migration help: {print_error}")
            })?;
            Ok(None)
        }
        Err(error) => Err(error.to_string()),
    }
}
