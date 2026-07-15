use agent_semantic_hook::{DecisionKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::registry;

#[test]
fn wrapper_raw_search_routes_to_provider_query_when_supported() {
    for command in [
        "DIRENV_SILENCE=1 direnv exec . rg -n WorkflowExecution src/lib.rs",
        "direnv exec . rg -n WorkflowExecution src/lib.rs",
        "rtk proxy rg -n WorkflowExecution src/lib.rs",
        "rtk run -c \"rg -n WorkflowExecution src/lib.rs\"",
        "bash -lc \"rg -n WorkflowExecution src/lib.rs\"",
        "zsh -lc \"rg -n WorkflowExecution src/lib.rs\"",
    ] {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({ "tool_name": "functions.exec_command", "tool_input": { "cmd": command } }),
        );
        assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
        assert_eq!(
            decision.reason_kind,
            ReasonKind::RawBroadSearch,
            "{command}"
        );
    }
}
