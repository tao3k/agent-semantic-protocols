pub(super) use super::common::{
    ClientHookConfig, DecisionKind, HookClassificationRequest, classify_hook_with_config, fs, json,
    load_client_config, load_client_config_for_project, registry, temp_root,
    with_direct_dispatch_roles,
};

#[path = "matching/materialization.rs"]
mod materialization;
#[path = "matching/policy_merge.rs"]
mod policy_merge;
