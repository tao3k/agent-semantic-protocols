//! Evaluates config-declared structured projection matchers after Bash parsing.

use agent_semantic_config::{
    HookClientStructuredFilterGrammar, HookClientStructuredProjectionMatchConfig,
};

use crate::executable::{ExecutableStatus, resolve_executable_with_status};
use crate::tool_action::ToolAction;

/// Match one structured projection contract and retain its typed source operands.
pub(super) fn match_source_operands(
    projection: &HookClientStructuredProjectionMatchConfig,
    action: &ToolAction,
) -> Option<Vec<String>> {
    let command = action.command.as_deref()?;
    let classification = match projection.filter_grammar {
        HookClientStructuredFilterGrammar::BoundedPathV1 => {
            crate::command_match::structured::classify_single_bounded_path_command(
                command,
                crate::command_match::structured::BoundedPathCommandSpecV1 {
                    binary: &projection.binary,
                    optional_subcommand_any: &projection.optional_subcommand_any,
                    option_any: &projection.option_any,
                    option_value_arity: &projection.option_value_arity,
                },
            )
        }
    };
    let source_operands = match classification {
        crate::command_match::structured::StructuredFilterClassificationV1::BoundedPath {
            source_operands,
            ..
        }
        | crate::command_match::structured::StructuredFilterClassificationV1::BoundedScalarPredicate {
            source_operands,
            ..
        } => source_operands,
        _ => return None,
    };
    if resolve_executable_with_status(&projection.binary).status != ExecutableStatus::Available {
        return None;
    }
    Some(source_operands)
}
