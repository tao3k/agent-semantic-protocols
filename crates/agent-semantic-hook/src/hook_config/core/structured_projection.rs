//! Evaluates config-declared structured projection matchers after Bash parsing.

use agent_semantic_config::{
    HookClientStructuredFilterGrammar, HookClientStructuredProjectionMatchConfig,
};

use crate::tool_action::ToolAction;

/// Match one structured projection contract and retain its typed source operands.
pub(super) fn match_source_operands(
    projection: &HookClientStructuredProjectionMatchConfig,
    action: &ToolAction,
) -> Option<Vec<String>> {
    let command = action.command.as_deref()?;
    let classification = match projection.filter_grammar {
        HookClientStructuredFilterGrammar::BoundedPathV1 => {
            crate::shell_parser::structured::classify_single_bounded_path_command(
                command,
                crate::shell_parser::structured::BoundedPathCommandSpec {
                    binary: &projection.binary,
                    optional_subcommand_any: &projection.optional_subcommand_any,
                    option_any: &projection.option_any,
                    option_value_arity: &projection.option_value_arity,
                    max_slice_items: projection.max_slice_items,
                },
            )
        }
    };
    let source_operands = match classification {
        crate::shell_parser::structured::StructuredFilterClassification::BoundedPath {
            source_operands,
            ..
        }
        | crate::shell_parser::structured::StructuredFilterClassification::BoundedScalarPredicate {
            source_operands,
            ..
        } => source_operands,
        _ => return None,
    };
    Some(source_operands)
}
