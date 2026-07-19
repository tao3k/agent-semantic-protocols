use std::borrow::Cow;

use serde::Serialize;

#[derive(Serialize)]
pub struct HostRequirement<'a> {
    pub platform: &'a str,
    #[serde(rename = "residentChildName")]
    pub resident_child_name: &'a str,
    #[serde(rename = "managedAgentKind")]
    pub managed_agent_kind: Cow<'a, str>,
    #[serde(rename = "requiredTransport")]
    pub required_transport: &'a str,
    #[serde(rename = "requiredOutputs")]
    pub required_outputs: &'a [&'a str],
    #[serde(rename = "blockedWhen")]
    pub blocked_when: &'a [&'a str],
}
