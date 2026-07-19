use std::borrow::Cow;

use serde::Serialize;

use crate::receipt::{LoopReceipt, TraceStep};
use crate::requirement::HostRequirement;

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
