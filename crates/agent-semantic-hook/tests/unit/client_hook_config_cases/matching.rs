pub(super) use super::common::{
    ClientHookConfig, DecisionKind, HookClassificationRequest, classify_hook_with_config, fs, json,
    load_client_config, load_client_config_for_project, registry, temp_root,
    with_required_resident_agents,
};

#[path = "matching/default_witnesses.rs"]
mod default_witnesses;
#[path = "matching/materialization.rs"]
mod materialization;
#[path = "matching/policy_merge.rs"]
mod policy_merge;
#[path = "matching/reasoning_dispatch.rs"]
mod reasoning_dispatch;
