/// Clap-owned project-registry command.
#[derive(Clone, Debug, Eq, PartialEq, clap::Parser)]
#[command(
    name = "asp cache projects",
    about = "Inspect and maintain ASP project state",
    disable_version_flag = true
)]
struct ProjectRegistryArgs {
    #[command(subcommand)]
    command: ProjectRegistryCommand,
}

#[derive(Clone, Debug, Eq, PartialEq, clap::Subcommand)]
enum ProjectRegistryCommand {
    /// Inspect or remove stale ASP project state.
    Gc(ProjectRegistryGcArgs),
}

/// Parsed arguments for project-registry garbage collection.
#[derive(Clone, Debug, Eq, PartialEq, clap::Args)]
pub(crate) struct ProjectRegistryGcArgs {
    /// Remove the eligible derived-state directories reported by this pass.
    #[arg(long)]
    pub(crate) apply: bool,

    /// Retain missing checkouts until they have been inactive for this many days.
    #[arg(long, default_value_t = 7)]
    pub(crate) grace_days: u64,
}

pub(crate) fn parse_project_registry_gc_args(
    args: &[String],
) -> Result<Option<ProjectRegistryGcArgs>, String> {
    let argv = std::iter::once("asp cache projects").chain(args.iter().map(String::as_str));
    match <ProjectRegistryArgs as clap::Parser>::try_parse_from(argv) {
        Ok(ProjectRegistryArgs {
            command: ProjectRegistryCommand::Gc(args),
        }) => Ok(Some(args)),
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
