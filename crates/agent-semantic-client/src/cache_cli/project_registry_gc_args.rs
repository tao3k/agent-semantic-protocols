//! Typed CLI arguments for State Home garbage collection.

/// Parsed arguments for State Home garbage collection.
#[derive(Clone, Debug, Eq, PartialEq, clap::Args)]
pub(crate) struct ProjectRegistryGcArgs {
    /// Remove the eligible derived-state directories reported by this pass.
    #[arg(long)]
    pub(crate) apply: bool,

    /// Retain inactive project state for this many days.
    #[arg(long, default_value_t = 7)]
    pub(crate) grace_days: u64,
}

/// Parsed arguments for temporary-workspace cache retirement.
#[derive(Clone, Debug, Eq, PartialEq, clap::Args)]
pub(crate) struct ProjectRegistryCleanArgs {
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
}

/// Clap-owned State Home garbage-collection command.
#[derive(Clone, Debug, Eq, PartialEq, clap::Parser)]
#[command(
    name = "gc",
    bin_name = "asp cache gc",
    about = "Inspect or remove stale derived state from the ASP State Home",
    disable_version_flag = true
)]
struct ProjectRegistryGcCli {
    #[command(flatten)]
    args: ProjectRegistryGcArgs,
}

/// Applying cleanup requires an explicit retention flag so a bare `clean`
/// invocation cannot delete state accidentally.
#[derive(Clone, Debug, Eq, PartialEq, clap::Parser)]
#[command(
    name = "clean",
    bin_name = "asp cache clean",
    about = "Retire cache for disappeared temporary workspaces after the retention window",
    disable_version_flag = true
)]
struct ProjectRegistryCleanCli {
    #[command(flatten)]
    args: ProjectRegistryCleanArgs,
}

/// Return the one command definition shared by execution parsing and help.
pub fn project_registry_gc_clap_command() -> clap::Command {
    <ProjectRegistryGcCli as clap::CommandFactory>::command()
}

/// Return the cleanup command definition shared by execution parsing and help.
pub fn project_registry_clean_clap_command() -> clap::Command {
    <ProjectRegistryCleanCli as clap::CommandFactory>::command()
}

pub(crate) fn parse_project_registry_gc_args(
    args: &[String],
) -> Result<Option<ProjectRegistryGcArgs>, String> {
    let argv = std::iter::once("asp cache gc").chain(args.iter().map(String::as_str));
    match project_registry_gc_clap_command().try_get_matches_from(argv) {
        Ok(matches) => {
            let parsed = <ProjectRegistryGcCli as clap::FromArgMatches>::from_arg_matches(&matches)
                .map_err(|error| error.to_string())?;
            Ok(Some(parsed.args))
        }
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            error
                .print()
                .map_err(|print_error| format!("failed to print cache GC help: {print_error}"))?;
            Ok(None)
        }
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) fn parse_project_registry_clean_args(
    args: &[String],
) -> Result<Option<ProjectRegistryCleanArgs>, String> {
    let argv = std::iter::once("asp cache clean").chain(args.iter().map(String::as_str));
    match project_registry_clean_clap_command().try_get_matches_from(argv) {
        Ok(matches) => {
            let parsed =
                <ProjectRegistryCleanCli as clap::FromArgMatches>::from_arg_matches(&matches)
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
