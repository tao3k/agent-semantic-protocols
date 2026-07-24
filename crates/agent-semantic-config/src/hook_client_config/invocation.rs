use serde::Deserialize;

use super::routing::{HookClientActionAuthority, HookClientActionKind};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentActionAuthorityRule {
    pub argv_prefix: Vec<String>,
    pub authority: HookClientActionAuthority,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentActionEffectRule {
    #[serde(default)]
    pub argv_prefix: Vec<String>,
    #[serde(default)]
    pub command_contains_any: Vec<String>,
    pub effect: HookClientActionKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientCommandWrapper {
    pub executable: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientInvocationShape {
    HostNative,
    Command,
    WrappedCommand,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientWrapperMatch {
    Matched,
    Unmatched,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientFlagPresence {
    Present,
    Absent,
}
