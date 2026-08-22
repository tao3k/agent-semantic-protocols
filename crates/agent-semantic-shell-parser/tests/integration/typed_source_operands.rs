use std::collections::BTreeMap;

use agent_semantic_shell_parser::structured::{
    BoundedPathCommandSpec, BoundedPathSegment, StructuredFilterClassification,
    classify_single_bounded_path_command,
};

#[test]
fn configured_projection_retains_only_the_typed_source_operand() {
    let optional_subcommands = Vec::new();
    let options = vec!["--raw-output".to_string()];
    let mut option_value_arity = BTreeMap::new();
    option_value_arity.insert("--arg".to_string(), 2);

    let classification = classify_single_bounded_path_command(
        "project-json --arg scope workspace .package.name package.json",
        BoundedPathCommandSpec {
            binary: "project-json",
            optional_subcommand_any: &optional_subcommands,
            option_any: &options,
            option_value_arity: &option_value_arity,
            max_slice_items: 64,
        },
    );

    let StructuredFilterClassification::BoundedPath {
        segments,
        source_operands,
    } = classification
    else {
        panic!("configured bounded projection must retain typed source operands");
    };
    assert_eq!(
        segments,
        [
            BoundedPathSegment::Field("package".to_string()),
            BoundedPathSegment::Field("name".to_string()),
        ]
    );
    assert_eq!(source_operands, ["package.json"]);
}
