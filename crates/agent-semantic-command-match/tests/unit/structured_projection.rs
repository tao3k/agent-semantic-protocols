use std::collections::BTreeMap;

use super::{
    BoundedPathCommandSpecV1, StructuredFilterClassificationV1,
    classify_single_bounded_path_command,
};

fn spec<'a>(
    binary: &'a str,
    subcommands: &'a [String],
    options: &'a [String],
    option_values: &'a BTreeMap<String, u8>,
) -> BoundedPathCommandSpecV1<'a> {
    BoundedPathCommandSpecV1 {
        binary,
        optional_subcommand_any: subcommands,
        option_any: options,
        option_value_arity: option_values,
    }
}

#[test]
fn command_model_is_configured_instead_of_binary_hardcoded() {
    let no_subcommands = Vec::new();
    let options = vec!["--raw-output".to_string()];
    let mut option_values = BTreeMap::new();
    option_values.insert("--arg".to_string(), 2);
    assert!(matches!(
        classify_single_bounded_path_command(
            "project-json --arg scope workspace .package.name package.json",
            spec("project-json", &no_subcommands, &options, &option_values),
        ),
        StructuredFilterClassificationV1::BoundedPath { .. }
    ));
}

#[test]
fn rejects_identity_multiple_inputs_and_multi_stage_commands() {
    let subcommands = vec!["eval".to_string(), "e".to_string()];
    let options = Vec::new();
    let option_values = BTreeMap::new();
    let configured = || spec("project-toml", &subcommands, &options, &option_values);
    assert_eq!(
        classify_single_bounded_path_command("project-toml eval . Cargo.toml", configured()),
        StructuredFilterClassificationV1::Identity
    );
    assert_eq!(
        classify_single_bounded_path_command(
            "project-toml .workspace Cargo.toml pyproject.toml",
            configured(),
        ),
        StructuredFilterClassificationV1::Compound
    );
    assert_eq!(
        classify_single_bounded_path_command(
            "project-toml .workspace Cargo.toml | sed Cargo.toml",
            configured(),
        ),
        StructuredFilterClassificationV1::Compound
    );
}
