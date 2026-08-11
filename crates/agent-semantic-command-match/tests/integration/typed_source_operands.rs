use std::collections::BTreeMap;

use agent_semantic_command_match::structured::{
    BoundedPathCommandSpecV1, BoundedPathSegmentV1, StructuredFilterClassificationV1,
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
        BoundedPathCommandSpecV1 {
            binary: "project-json",
            optional_subcommand_any: &optional_subcommands,
            option_any: &options,
            option_value_arity: &option_value_arity,
            max_slice_items: 64,
        },
    );

    let StructuredFilterClassificationV1::BoundedPath {
        segments,
        source_operands,
    } = classification
    else {
        panic!("configured bounded projection must retain typed source operands");
    };
    assert_eq!(
        segments,
        [
            BoundedPathSegmentV1::Field("package".to_string()),
            BoundedPathSegmentV1::Field("name".to_string()),
        ]
    );
    assert_eq!(source_operands, ["package.json"]);
}
