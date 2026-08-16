use clap::{Arg, Command};

#[derive(Debug)]
pub(super) struct ExactQueryArgs {
    pub(super) structural_selector: String,
    pub(super) projection: String,
}

impl ExactQueryArgs {
    fn validate(self) -> Result<Self, String> {
        if matches!(self.projection.as_str(), "source" | "callable-skeleton") {
            Ok(self)
        } else {
            Err(format!("unsupported exact projection: {}", self.projection))
        }
    }
}

pub(super) fn parse_exact_query_args(provider_args: &[String]) -> Result<ExactQueryArgs, String> {
    let matches = exact_query_command()
        .try_get_matches_from(provider_args)
        .map_err(|error| error.to_string())?;
    let exact = ExactQueryArgs {
        projection: matches
            .get_one::<String>("projection")
            .expect("required exact projection")
            .clone(),
        structural_selector: matches
            .get_one::<String>("selector")
            .expect("required exact selector")
            .clone(),
    };
    exact.validate()
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
                .value_parser(["source", "callable-skeleton"])
                .num_args(1),
        )
}
