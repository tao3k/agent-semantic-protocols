use std::borrow::Cow;

use serde::Serialize;

use crate::receipt::{LoopReceipt, TraceStep};
use crate::requirement::HostRequirement;

/// One executable command that enters an interactive resident loop.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResidentInteractiveCommand {
    #[serde(rename = "schemaId")]
    pub schema_id: &'static str,
    #[serde(rename = "schemaVersion")]
    pub schema_version: &'static str,
    pub argv: Vec<String>,
}

impl ResidentInteractiveCommand {
    pub fn bootstrap(resident_name: &str, root_session_id: Option<&str>) -> Self {
        Self::bootstrap_with_dispatch(resident_name, root_session_id, None, None)
    }

    #[must_use]
    pub fn bootstrap_with_dispatch(
        resident_name: &str,
        root_session_id: Option<&str>,
        receipt_kind: Option<&str>,
        command_json: Option<&str>,
    ) -> Self {
        let mut argv = vec![
            "asp".to_string(),
            "agent".to_string(),
            "session".to_string(),
            "bootstrap".to_string(),
            "--name".to_string(),
            resident_name.to_string(),
        ];
        if let Some(root_session_id) = root_session_id.filter(|value| !value.is_empty()) {
            argv.extend(["--root-session-id".to_string(), root_session_id.to_string()]);
        }
        if let Some(receipt_kind) = receipt_kind {
            argv.extend(["--receipt-kind".to_string(), receipt_kind.to_string()]);
        }
        if let Some(command_json) = command_json {
            argv.extend(["--command-json".to_string(), command_json.to_string()]);
        }
        Self {
            schema_id: "agent.semantic-protocols.loop.resident-interactive-command",
            schema_version: "1",
            argv,
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/choice.rs"]
mod tests;

#[derive(Serialize)]
pub struct ChoicePane<'a, State>
where
    State: Serialize,
{
    #[serde(rename = "schemaId")]
    pub schema_id: &'a str,
    #[serde(rename = "schemaVersion")]
    pub schema_version: &'a str,
    pub owner: &'a str,
    pub state: State,
    pub name: &'a str,
    #[serde(rename = "hostRequirement")]
    pub host_requirement: HostRequirement<'a>,
    pub trace: Vec<TraceStep<'a, State>>,
    pub choices: Vec<Choice<'a, State>>,
    pub receipt: LoopReceipt<'a>,
}

#[derive(Serialize)]
pub struct Choice<'a, State>
where
    State: Serialize,
{
    pub id: &'a str,
    pub label: &'a str,
    #[serde(rename = "platformAction")]
    pub platform_action: Cow<'a, str>,
    #[serde(rename = "nextState")]
    pub next_state: State,
    #[serde(rename = "requiredInputs")]
    pub required_inputs: &'a [&'a str],
}
