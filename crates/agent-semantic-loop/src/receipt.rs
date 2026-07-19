use serde::Serialize;

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
