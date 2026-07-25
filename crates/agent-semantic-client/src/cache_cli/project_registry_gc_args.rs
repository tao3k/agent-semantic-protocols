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

/// Return the one command definition shared by execution parsing and help.
pub fn project_registry_gc_clap_command() -> clap::Command {
    <ProjectRegistryGcCli as clap::CommandFactory>::command()
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
