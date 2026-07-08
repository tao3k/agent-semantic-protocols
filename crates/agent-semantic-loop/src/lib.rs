use serde::Serialize;

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
    pub platform_action: &'a str,
    #[serde(rename = "nextState")]
    pub next_state: State,
    #[serde(rename = "requiredInputs")]
    pub required_inputs: &'a [&'a str],
}

#[derive(Serialize)]
pub struct HostRequirement<'a> {
    pub platform: &'a str,
    #[serde(rename = "residentChildName")]
    pub resident_child_name: &'a str,
    #[serde(rename = "managedAgentKind")]
    pub managed_agent_kind: &'a str,
    #[serde(rename = "requiredTransport")]
    pub required_transport: &'a str,
    #[serde(rename = "requiredOutputs")]
    pub required_outputs: &'a [&'a str],
    #[serde(rename = "blockedWhen")]
    pub blocked_when: &'a [&'a str],
}

#[derive(Serialize)]
pub struct TraceStep<'a, State>
where
    State: Serialize,
{
    pub state: State,
    pub result: &'a str,
}

#[derive(Serialize)]
pub struct LoopReceipt<'a> {
    pub loop_name: &'a str,
    pub invariant: &'a str,
    #[serde(rename = "noNextCommand")]
    pub no_next_command: bool,
}
