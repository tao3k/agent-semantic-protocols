//! Refines agent-facing recovery text without changing policy decisions.

use crate::HookDecision;

pub(crate) fn with_selector_only_subagent_message(mut decision: HookDecision) -> HookDecision {
    if decision
        .message
        .contains("Return one compact `[asp-search-subagent]` graph-route receipt")
        && !decision
            .message
            .contains("Return selector-only `[asp-search-subagent]` evidence")
    {
        decision.message = decision.message.replace(
            "Return one compact `[asp-search-subagent]` graph-route receipt",
            "Return selector-only `[asp-search-subagent]` evidence. Return one compact `[asp-search-subagent]` graph-route receipt",
        );
    }
    decision
}
