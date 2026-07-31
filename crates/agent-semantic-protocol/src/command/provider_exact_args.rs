use clap::{Arg, ArgAction, Command, builder::PossibleValuesParser};

#[derive(Debug)]
pub(super) struct ExactQueryArgs {
    pub(super) structural_selector: String,
    pub(super) projection: String,
    pub(super) json: bool,
}

pub(super) fn parse_exact_query_args(provider_args: &[String]) -> Result<ExactQueryArgs, String> {
    let matches = exact_query_command()
        .try_get_matches_from(provider_args)
        .map_err(|error| error.to_string())?;
    Ok(ExactQueryArgs {
        structural_selector: matches
            .get_one::<String>("selector")
            .expect("required exact selector")
            .clone(),
        projection: matches
            .get_one::<String>("projection")
            .expect("required exact projection")
            .clone(),
        json: matches.get_flag("json"),
    })
}

pub(super) fn exact_query_command() -> Command {
    Command::new("query")
        .disable_help_flag(true)
        .disable_version_flag(true)
        .arg(
            Arg::new("selector")
                .long("selector")
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new("projection")
                .long("projection")
                .required(true)
                .num_args(1)
                .value_parser(PossibleValuesParser::new(["source", "callable-skeleton"])),
        )
        .arg(Arg::new("json").long("json").action(ArgAction::SetTrue))
}
