//! Evaluates config-declared structured projection matchers after Bash parsing.

use agent_semantic_config::{
    HookClientStructuredFilterGrammar, HookClientStructuredProjectionMatchConfig,
};

use crate::executable::{ExecutableStatus, resolve_executable_with_status};
use crate::tool_action::ToolAction;

/// Match one structured projection contract against the parsed tool action.
pub(super) fn matches(
    projection: Option<&HookClientStructuredProjectionMatchConfig>,
    action: &ToolAction,
) -> bool {
    let Some(projection) = projection else {
        return true;
    };
    let Some(command) = action.command.as_deref() else {
        return false;
    };
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
    if !matches!(
        classification,
        crate::command_match::structured::StructuredFilterClassificationV1::BoundedPath { .. }
    ) {
        return false;
    }
    resolve_executable_with_status(&projection.binary).status == ExecutableStatus::Available
}
